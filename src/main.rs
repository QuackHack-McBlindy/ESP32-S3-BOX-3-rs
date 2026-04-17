#![no_std]
#![no_main]
#![allow(warnings)]
#![allow(non_snake_case)]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]


use alloc::vec;
use esp_println as _;
use defmt::{info, Debug2Format, error};
use core::sync::atomic::{AtomicU8, AtomicI8, AtomicU32, AtomicI32, AtomicBool, Ordering};
use core::net::SocketAddr;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::mutex::Mutex;

use esp_hal::{
    Async,
    main,
    Blocking,
    dma_buffers,
    dma::{DmaRxBuf, DmaDescriptor},
    analog::adc::{Adc, AdcConfig, Attenuation},
    clock::CpuClock,
    delay::Delay,
    gpio::{Level, NoPin, Output, OutputConfig, OutputSignal, Input, InputConfig, Pull, Pin, Flex},
    peripherals::{ADC1, GPIO17, GPIO45},    
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{I2s, I2sRx, I2sTx, Config as I2sConfig, DataFormat, Channels, I2sInterrupt},
    i2s::master::asynch::I2sWriteDmaTransferAsync,
    i2s::master::asynch::I2sReadDmaTransferAsync,
    spi::master::{Config as SpiConfig, Spi},
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
    ledc::channel::Channel,
    time::{Instant, Rate},
    timer::timg::TimerGroup,
    rng::Rng,
};


// i2C/SPI bus sharing
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::i2c::I2c as HalI2c;

// WiFi / METWORK
use esp_radio::wifi::{ClientConfig, ModeConfig, Config as WifiConfig};
use embassy_net::{Config as NetConfig, DhcpConfig, Stack, StackResources, Runner, dns::DnsQueryType, tcp::TcpSocket, IpAddress};
use tinyapi::*;

// display
use display_interface_spi::SPIInterface;
use ili9341::{DisplaySize240x320, Ili9341, Orientation};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Point},
    mono_font::{
        iso_8859_1::FONT_8X13,
        MonoTextStyle, MonoTextStyleBuilder,
    },
    pixelcolor::{Rgb565, RgbColor},
    text::{Alignment, Text},
    Drawable,
};


// LOAD MODULES
mod macros; // best first
use macros::*;
mod es7210; // audio codec for mic
mod es8311; // audio codec for speaker
mod aht20; // temperature & humidity sensor
mod mic;
mod microphone;
mod speaker;
use speaker::*;
mod media;
use media::*;
mod buttons;
use buttons::top_left_button_task;
mod presence;
mod wifi;
use wifi::{CURRENT_RSSI, connection, net_task};
// API (must be last)
mod api;
use api::*;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// PSRAM?
extern crate alloc;
use alloc::boxed::Box;
use alloc::format;

// bootloader
esp_bootloader_esp_idf::esp_app_desc!(
  
);

// COMPILE-TIME ENVIORMENT VARIABLES
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");
const BACKEND_TCP_HOST: &str = env!("BACKEND_TCP_HOST");
const FW_VERSION: &str = env!("CARGO_PKG_VERSION");


use esp_hal::peripherals::GPIO2;

const SAMPLE_RATE: u32 = 16000;
const BUFFER_SIZE: usize = 4096;
const SAMPLE_COUNT: usize = 256;


use esp_hal::peripherals::I2C0;
pub static BACKLIGHT_PERCENT: AtomicU8 = AtomicU8::new(0);

pub type I2cBus = I2c<'static, Blocking>;
pub static I2C_BUS: CsMutex<RefCell<Option<I2cBus>>> = CsMutex::new(RefCell::new(None));

pub static ES7210: CsMutex<RefCell<Option<es7210::Es7210>>> = CsMutex::new(RefCell::new(None));
pub static ES8311: CsMutex<RefCell<Option<es8311::Es8311>>> = CsMutex::new(RefCell::new(None));

pub static BATTERY_VOLTAGE: AtomicU32 = AtomicU32::new(0);
pub static BATTERY_PERCENT: AtomicU8 = AtomicU8::new(100);
pub static RSSI: AtomicI32 = AtomicI32::new(0);

#[embassy_executor::task]
async fn backlight_task(mut channel: &'static mut Channel<'static, LowSpeed>) {
    loop {
        let percent = BACKLIGHT_PERCENT.load(Ordering::Relaxed);
        channel.set_duty(percent).unwrap();
        Timer::after(Duration::from_millis(100)).await;
    }
}

use embassy_net::Ipv4Address;
pub static CURRENT_IP: AtomicU32 = AtomicU32::new(0);

pub static MIC_VOLUME: AtomicU8 = AtomicU8::new(72);
pub static SPEAKER_VOLUME: AtomicU8 = AtomicU8::new(58);
pub static MIC_MUTED: AtomicBool = AtomicBool::new(false);
pub static SPEAKER_MUTED: AtomicBool = AtomicBool::new(false);

fn mic_volume_percent_to_db(percent: u8) -> i8 {
    let clamped = percent.clamp(0, 100) as i32;
    let db = -95 + (clamped * 127) / 100;
    db as i8
}

fn speaker_volume_percent(percent: u8) -> u8 {
    percent.clamp(0, 100)
}

#[embassy_executor::task]
pub async fn audio_settings_task(i2c_bus: &'static CsMutex<RefCell<I2cBus>>) {
    let mut last_mic_vol = MIC_VOLUME.load(Ordering::Relaxed);
    let mut last_spk_vol = SPEAKER_VOLUME.load(Ordering::Relaxed);
    let mut last_mic_muted = MIC_MUTED.load(Ordering::Relaxed);
    let mut last_spk_muted = SPEAKER_MUTED.load(Ordering::Relaxed);

    loop {
        let mic_vol = MIC_VOLUME.load(Ordering::Relaxed);
        let spk_vol = SPEAKER_VOLUME.load(Ordering::Relaxed);
        let mic_muted = MIC_MUTED.load(Ordering::Relaxed);
        let spk_muted = SPEAKER_MUTED.load(Ordering::Relaxed);

        let mic_changed = mic_vol != last_mic_vol || mic_muted != last_mic_muted;
        let spk_changed = spk_vol != last_spk_vol || spk_muted != last_spk_muted;

        if mic_changed || spk_changed {
            critical_section::with(|cs| {
                let mut i2c_dev = CriticalSectionDevice::new(i2c_bus);

                if mic_changed {
                    let mut es7210_borrow = ES7210.borrow_ref_mut(cs);
                    if let Some(es7210) = es7210_borrow.as_mut() {
                        if mic_muted != last_mic_muted {
                            if let Err(e) = es7210.set_mute(&mut i2c_dev, mic_muted) {
                                info!("ES7210 mute failed: {:?}", Debug2Format(&e));
                            }
                        }

                        if mic_vol != last_mic_vol {
                            let db = mic_volume_percent_to_db(mic_vol);
                            if let Err(e) = es7210.gain_set(&mut i2c_dev, db) {
                                info!("ES7210 gain set failed: {:?}", Debug2Format(&e));
                            }
                        }

                        last_mic_vol = mic_vol;
                        last_mic_muted = mic_muted;
                    }
                }

                if spk_changed {
                    let mut es8311_borrow = ES8311.borrow_ref_mut(cs);
                    if let Some(es8311) = es8311_borrow.as_mut() {
                        if spk_muted != last_spk_muted {
                            if let Err(e) = es8311.mute(&mut i2c_dev, spk_muted) {
                                info!("ES8311 mute failed: {:?}", Debug2Format(&e));
                            }
                        }

                        if spk_vol != last_spk_vol {
                            let vol = speaker_volume_percent(spk_vol);
                            if let Err(e) = es8311.volume_set(&mut i2c_dev, vol, None) {
                                info!("ES8311 volume set failed: {:?}", Debug2Format(&e));
                            }
                        }

                        last_spk_vol = spk_vol;
                        last_spk_muted = spk_muted;
                    }
                }
            });
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

use tinyapi::http_get;

// MAIN
#[allow(clippy::large_stack_frames)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);
    info!("Started ESP32-S3-BOX-3 (version {})", FW_VERSION);
   

    // GPIO PINS
    let lcd_clk = peripherals.GPIO7;
    let lcd_mosi = peripherals.GPIO6;
    let lcd_cs = peripherals.GPIO5;
    let lcd_dc = peripherals.GPIO4;
    let lcd_rst = peripherals.GPIO48;
    let backlight = peripherals.GPIO47;
    let touch_int = peripherals.GPIO3;

    let i2c_a_sda = peripherals.GPIO8;
    let i2c_a_scl = peripherals.GPIO18;
    let i2c_b_sda = peripherals.GPIO41;
    let i2c_b_scl = peripherals.GPIO40;

    let i2s_bclk = peripherals.GPIO17;
    let i2s_lrclk = peripherals.GPIO45;
    let i2s_bclk_rx = unsafe { GPIO17::steal() };
    let i2s_lrclk_rx = unsafe { GPIO45::steal() };          
    let i2s_mclk = peripherals.GPIO2;
    let i2s_mclk_rx = unsafe { GPIO2::steal() };          
    let i2s_din = peripherals.GPIO16;
    let i2s_dout = peripherals.GPIO15;

    let mut flex_bclk = Flex::new(i2s_bclk);
    let mut flex_lrclk = Flex::new(i2s_lrclk);
    let mut flex_mclk = Flex::new(i2s_mclk);    

    let (input_lrclk, output_lrclk) = flex_lrclk.split();
    let (input_mclk, output_mclk) = flex_mclk.split();
    let (input_bclk, output_bclk) = flex_bclk.split();
    

    let button_top_left = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up)
    );

    // pa_enable.set_low() to mute
    let mut pa_enable = Output::new(
        peripherals.GPIO46,
        Level::High,
        OutputConfig::default()
    );
    
    let mut occupancy = Input::new(
        peripherals.GPIO21,
        InputConfig::default().with_pull(Pull::Down)
    );

    // ADC / Battery
    let mut adc_config = AdcConfig::new();
    let battery_pin = peripherals.GPIO10;
    let mut adc_pin = adc_config.enable_pin(battery_pin, Attenuation::_0dB);
    let mut adc = Adc::new(peripherals.ADC1, adc_config);

    // I2C BUS A
    let mut i2c_a = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(i2c_a_sda)
    .with_scl(i2c_a_scl);    


    // I2C BUS B
    let mut i2c_b = I2c::new(
        peripherals.I2C1,
        I2cConfig::default().with_frequency(Rate::from_khz(50)),
    )
    .unwrap()
    .with_sda(i2c_b_sda)
    .with_scl(i2c_b_scl);

    // LOCK & SHARE BUSSES
    let i2c_a_mutex = Box::leak(Box::new(CsMutex::new(RefCell::new(i2c_a))));
    let i2c_b_mutex = Box::leak(Box::new(CsMutex::new(RefCell::new(i2c_b))));

    // Audio codecs
    let es7210 = es7210::Es7210::new(0x40);
    let es8311 = es8311::Es8311::new(0x18);

    { // configure audio codecs
        let mut i2c = CriticalSectionDevice::new(&i2c_a_mutex);

        // ES7210 (ADC)
        let codec_cfg = es7210::CodecConfig {
            sample_rate_hz: 16000,
            mclk_ratio: 256,
            i2s_format: es7210::I2sFormat::I2S,
            bit_width: es7210::I2sBits::Bits16,
            mic_bias: es7210::MicBias::V2_87,
            mic_gain: es7210::MicGain::Gain30dB,
            tdm_enable: false,
        };
        match es7210.config_codec(&mut i2c, &codec_cfg) {
            Ok(()) => info!("ES7210 initialized successfully"),
            Err(e) => info!("ES7210 init failed: {:?}", Debug2Format(&e)),
        }
        if let Err(e) = es7210.gain_set(&mut i2c, 20) {
            info!("ES7210 volume set failed: {:?}", Debug2Format(&e));
        }
        if let Err(e) = es7210.set_mute(&mut i2c, false) {
            info!("Failed to configure ES7210 mute status {:?}", Debug2Format(&e));
        }
        
        // ES8311 (DAC)
        let clock_cfg = es8311::ClockConfig {
            mclk_inverted: false,
            sclk_inverted: false,
            mclk_from_mclk_pin: true,
            mclk_frequency: 4096000,
            sample_frequency: 16000,
        };
        match es8311.init(
            &mut i2c,
            &clock_cfg,
            es8311::Resolution::Bits16,
            es8311::Resolution::Bits16,
        ) {
            Ok(()) => info!("ES8311 initialised successfully"),
            Err(e) => info!("ES8311 init failed: {:?}", Debug2Format(&e)),
        }
        let _ = es8311.volume_set(&mut i2c, 50, None);
        let _ = es8311.mute(&mut i2c, false);
    } // release i2c


    
    // LEDC / Backlight
    let mut ledc = mk_static!(Ledc, Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    
    // low speed timer (Timer0) for 24 kHz with 10‑bit duty resolution
    let lstimer0 = mk_static!(
        esp_hal::ledc::timer::Timer<'static, LowSpeed>,
        ledc.timer::<LowSpeed>(esp_hal::ledc::timer::Number::Timer0)
    );
    lstimer0
        .configure(esp_hal::ledc::timer::config::Config {
            duty: esp_hal::ledc::timer::config::Duty::Duty10Bit,
            clock_source: esp_hal::ledc::timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();
    
    // create a channel and assign it to the timer and GPIO 47
    let mut channel0 = ledc.channel(
        esp_hal::ledc::channel::Number::Channel0,
        backlight,
    );
    channel0
        .configure(esp_hal::ledc::channel::config::Config {
            timer: lstimer0,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();
    
    // leak the channel to get static mut
    let backlight_channel: &'static mut _ = Box::leak(Box::new(channel0)); 
    
    // DISPLAY
    let spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(40))
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_sck(lcd_clk)
    .with_mosi(lcd_mosi)
    .with_miso(NoPin);

    let cs = Output::new(lcd_cs, Level::High, OutputConfig::default());
    let dc = Output::new(lcd_dc, Level::Low, OutputConfig::default());
    let rst = Output::new(lcd_rst, Level::Low, OutputConfig::default());
  
    let mut delay_spi = Delay::new();
    let mut delay_display = Delay::new();
    
    let spi_device = ExclusiveDevice::new(spi_bus, cs, delay_spi).unwrap();
    let interface = SPIInterface::new(spi_device, dc);
    
    let mut display = Ili9341::new(
        interface,
        rst,
        &mut delay_display,
        Orientation::Portrait,
        DisplaySize240x320,
    ).unwrap();
    
    display.clear(Rgb565::BLACK).unwrap();
 

    // WIFI Setup    
    let radio = &*mk_static!(esp_radio::Controller<'static>, esp_radio::init().expect("WiFi - ❌ Failed to initialize controller"));
    
    let client_config = ClientConfig::default()
        .with_ssid(SSID.into())
        .with_password(PASSWORD.into());
    let mode_config = ModeConfig::Client(client_config);
    let radio_config = WifiConfig::default();
    
    let (mut wifi_controller, interfaces) = esp_radio::wifi::new(
        radio,
        peripherals.WIFI,
        radio_config,
    )
    .expect("Wi‑Fi - ❌ Failed to initialize Wi-Fi controller");
        
    // spawn WiFi task
    spawner.spawn(connection(wifi_controller)).unwrap();
    
    // embassy-net setup
    let net_config = NetConfig::dhcpv4(DhcpConfig::default());
    let rng = Rng::new();
    let seed = (u64::from(rng.random())) << 32 | u64::from(rng.random());
    
    let stack_resources = mk_static!(StackResources<16>, StackResources::<16>::new());
    
    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        net_config,
        stack_resources,
        seed,
    );
    let stack = mk_static!(Stack<'static>, stack);

    
    spawner.spawn(net_task(runner)).unwrap();
    
    stack.wait_link_up().await;
    stack.wait_config_up().await;
    
    let ip = loop {
        if let Some(config) = stack.config_v4() {
            break config.address;
        }
        Timer::after(Duration::from_millis(500)).await;
    };
    let ip_addr = ip.address();
    let ip_raw = u32::from(ip_addr);
    CURRENT_IP.store(ip_raw, Ordering::Relaxed);
    info!("IP: {}", ip_addr);
    
    


    // I2S Audio setup 
    // DMA buffers
    
    // I2S Audio setup 
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(BUFFER_SIZE);
    
    let i2s_config = I2sConfig::default()
        .with_sample_rate(Rate::from_hz(16_000))
        .with_data_format(DataFormat::Data16Channel16)
        .with_channels(Channels::STEREO);
    
    // Create the I2S instance (moves peripherals.I2S0)
    let mut i2s = I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        i2s_config,
    ).unwrap() 
    .with_mclk(output_mclk)
    .into_async();
    
    Timer::after(Duration::from_millis(10)).await;
    // steal provides access    
    let i2s0 = unsafe { esp_hal::peripherals::I2S0::steal() };
    let i2s_regs = i2s0.register_block();

    // set rx_slave_mod in RX_CONF
    i2s_regs.rx_conf().modify(|_, w| w.rx_slave_mod().set_bit());
    // set the signal loopback flag in TX_CONF_REG
    i2s_regs.tx_conf().modify(|_, w| w.sig_loopback().set_bit());

    let status = i2s_regs.int_st().read();
    info!("I2S interrupt status: raw = 0x{:08X}", status.bits());
    

    #[cfg(feature = "use_mic")]
    {
        let BACKEND_TCP_PORT: u16 = env!("BACKEND_TCP_PORT").parse().expect("Invalid port");
        // resolve backend address
        let remote_addr = loop {
            match stack.dns_query(BACKEND_TCP_HOST, DnsQueryType::A).await {
                Ok(addr) => break (addr[0], BACKEND_TCP_PORT).into(),
                Err(e) => {
                    info!("DNS lookup error for {}: {}", BACKEND_TCP_HOST, e);
                    Timer::after(Duration::from_secs(5)).await;
                }
            }
        };
     
        let i2s_rx = i2s.i2s_rx
            .with_din(i2s_din)
            .build(rx_descriptors);
        spawner.spawn(microphone::audio_capture_task(i2s_rx, stack, remote_addr)).unwrap();
    }

    Timer::after(Duration::from_millis(109)).await;

    #[cfg(feature = "use_speaker")]
    {
        let i2s_tx = i2s.i2s_tx
            .with_bclk(output_bclk)
            .with_ws(output_lrclk)
            .with_dout(i2s_dout)
            .build(tx_descriptors);
        let i2s_tx: &'static mut _ = Box::leak(Box::new(i2s_tx));
        spawner.spawn(speaker_task(i2s_tx)).unwrap();
        let BACKEND_TCP_PORT: u16 = env!("BACKEND_TCP_PORT").parse().expect("Invalid port");
        spawner.spawn(audio_playback_task(stack, BACKEND_TCP_PORT)).unwrap();        
        spawner.spawn(top_left_button_task(button_top_left)).unwrap();
    }

    Timer::after(Duration::from_millis(1000)).await;

    // init API routes
    api::init_routes().await;

    // TASKS
    let _ = spawner;

    spawner.spawn(backlight_task(backlight_channel)).unwrap();
    // start API on port 80
    spawner.spawn(tinyapi::web_server_task(stack)).unwrap();

    // sensors
    spawner.spawn(aht20::sensor_task(i2c_b_mutex)).unwrap();
    // motion
    spawner.spawn(presence::occupancy_task(occupancy)).unwrap();
    // sync time
    //spawner.spawn(ntp::ntp_task(stack)).unwrap();


    loop { // calculate battery %
        let raw = adc.read_blocking(&mut adc_pin);
        let pin_voltage = raw as f32 * 1100.0 / 4095.0 / 1000.0;
        let battery_voltage = pin_voltage * 4.11;
        let percentage = ((battery_voltage - 3.0) / (4.2 - 3.0) * 100.0)
            .clamp(0.0, 100.0) as u8;

        // store as millivolts (u32) no go float
        let voltage_mv = (battery_voltage * 1000.0) as u32;
        BATTERY_VOLTAGE.store(voltage_mv, Ordering::Relaxed);
        BATTERY_PERCENT.store(percentage, Ordering::Relaxed);

        let rssi = wifi::CURRENT_RSSI.load(Ordering::Relaxed);
        RSSI.store(rssi, Ordering::Relaxed);
        let emoji = match percentage {
            0..=10 => "🪫⚡",
            11..=29 => "🪫",
            30..=70 => "🔋",
            _ => "🔋",
        };
        info!("{} {}%,  ({} mV)", emoji, percentage, voltage_mv);
        info!("🛜 {} dBm", rssi);
    
        Timer::after(Duration::from_secs(60)).await;
    }
}

