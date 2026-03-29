#![no_std]
#![no_main]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]

use esp_println as _;
use defmt::{info, Debug2Format};
use core::sync::atomic::Ordering;

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
    gpio::{Level, NoPin, Output, OutputConfig, Input, InputConfig, Pull},
    peripherals::ADC1,
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{I2s, I2sRx, I2sTx, Config as I2sConfig, DataFormat, Channels},
    i2s::master::asynch::I2sWriteDmaTransferAsync,
    i2s::master::asynch::I2sReadDmaTransferAsync,
    spi::master::{Config as SpiConfig, Spi},
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
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
use embassy_net::{Config as NetConfig, DhcpConfig, StackResources, Runner, dns::DnsQueryType, tcp::TcpSocket};
use esp_radio::wifi::{ClientConfig, ModeConfig, Config as WifiConfig};

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
mod es7210; // audio codec for mic
mod es8311; // audio codec for speaker
mod aht20; // temperature & humidity sensor
mod audio_input; // microphones stream
mod buttons; // physical buttons
mod presence; // presence sensor
mod wifi; // network stack
use wifi::{CURRENT_RSSI, connection, net_task};
mod macros; // helpers
use macros::*;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// PSRAM?
extern crate alloc;
use alloc::boxed::Box;

// bootloader
esp_bootloader_esp_idf::esp_app_desc!();

// COMPILE-TIME ENVIORMENT VARIABLES
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");
const BACKEND_TCP_HOST: &str = env!("BACKEND_TCP_HOST");
const BACKEND_TCP_PORT: u16 = env!("BACKEND_TCP_PORT").parse().expect("Invalid port");

const SAMPLE_RATE: u32 = 16000;
const BUFFER_SIZE: usize = 4096;
const SAMPLE_COUNT: usize = 256;


async fn play_boot_sound(
    i2s_tx: &mut I2sTx<'_, Async>,
    tx_buffer: &mut [u8; BUFFER_SIZE],
) {
    const SAMPLE_RATE: f32 = 16000;
    const FREQ: f32 = 880.0;
    let mut phase = 0.0f32;

    for chunk in tx_buffer.chunks_mut(4) {
        let sample =
            (libm::sinf(phase * 2.0 * core::f32::consts::PI)
                * i16::MAX as f32 * 0.2) as i16; // 20% multiplier

        chunk[0] = (sample & 0xFF) as u8;
        chunk[1] = (sample >> 8) as u8;
        chunk[2] = (sample & 0xFF) as u8;
        chunk[3] = (sample >> 8) as u8;

        phase += FREQ / SAMPLE_RATE;
        if phase >= 1.0 {
            phase -= 1.0;
        }
    }
    if let Err(e) = i2s_tx.write_dma_async(tx_buffer).await {
        defmt::error!("TX error: {:?}", e);
    }
}

 
// MAIN
#[allow(clippy::large_stack_frames)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);
    info!("Started ESP32-S3-BOX-3!");


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
    let i2s_mclk = peripherals.GPIO2;
    let i2s_din = peripherals.GPIO16;
    let i2s_dout = peripherals.GPIO15;

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
        let _ = es8311.volume_set(&mut i2c, 80, None);
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
    
    let stack_resources = mk_static!(StackResources<3>, StackResources::<3>::new());
    
    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        net_config,
        stack_resources,
        seed,
    );
    
    spawner.spawn(net_task(runner)).unwrap();
    
    stack.wait_link_up().await;
    stack.wait_config_up().await;
    
    let ip = loop {
        if let Some(config) = stack.config_v4() {
            break config.address;
        }
        Timer::after(Duration::from_millis(500)).await;
    };
    info!("IP: {}", ip);
    
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

    
    // I2S Audio setup 
    // DMA buffers
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
        dma_buffers!(BUFFER_SIZE);

    let i2s_config = I2sConfig::default()
        .with_sample_rate(Rate::from_hz(16_000))
        .with_data_format(DataFormat::Data16Channel16);
    
    let mut i2s = I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        i2s_config,
    ).unwrap()
    .with_mclk(i2s_mclk)
    .into_async();

    let mut i2s_tx = i2s.i2s_tx
        .with_dout(i2s_dout)
        .build(tx_descriptors);

    let mut i2s_rx = i2s.i2s_rx
        .with_bclk(i2s_bclk)
        .with_ws(i2s_lrclk)    
        .with_din(i2s_din)
        .build(rx_descriptors);

    Timer::after(Duration::from_millis(100)).await;
    play_boot_sound(&mut i2s_tx, tx_buffer).await;

       
    // TASKS
    let _ = spawner;

    // sensors
    spawner.spawn(aht20::sensor_task(i2c_b_mutex)).unwrap();
    // motion
    spawner.spawn(presence::occupancy_task(occupancy)).unwrap();
    // buttons
    spawner.spawn(buttons::top_left_button_task(button_top_left)).unwrap();
    // microphones
    spawner.spawn(audio_input::audio_capture_task(i2s_rx, stack, remote_addr, backlight_channel)).unwrap();

    
    loop { // calculate battery %
        let raw = adc.read_blocking(&mut adc_pin);
        let pin_voltage = raw as f32 * 1100.0 / 4095.0 / 1000.0;
        let battery_voltage = pin_voltage * 4.11;
        let percentage = ((battery_voltage - 3.0) / (4.2 - 3.0) * 100.0)
            .clamp(0.0, 100.0) as u8;

        let emoji = match percentage {
            0..=10 => "🪫⚡",
            11..=29 => "🪫",
            30..=70 => "🔋",
            _ => "🔋",
        };
        info!("{} {}%,  ({=f32} V)", emoji, percentage, battery_voltage);
        
        // show RSSI 
        let rssi = wifi::CURRENT_RSSI.load(Ordering::Relaxed);
        info!("🛜 {} dBm", rssi);
        Timer::after(Duration::from_secs(60)).await; // every minute
    }
}
