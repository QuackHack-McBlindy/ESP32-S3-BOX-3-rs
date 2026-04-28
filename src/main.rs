// ESP32-S3-BOX-3-rs https://github.com/QuackHack-McBlindy/ESP32-S3-BOX-3-rs
// BARE METAL NO_STD
// VOICE ASSISTANT FIRMWARE
#![no_std]
#![no_main]
// NOBODY TELLS ME WHAT TO DO!
//#![allow(warnings)]
#![allow(non_snake_case)]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]

// IMPORTS
use esp_println as _;
use defmt::{info, Debug2Format, error};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

// HARDWARE ABSTRACTION LAYER IMPORTS
use esp_hal::{
    Blocking,
    dma_buffers,
    analog::adc::{Adc, AdcConfig, Attenuation},
    clock::CpuClock,
    delay::Delay,
    gpio::{Level, NoPin, Output, OutputConfig, Input, InputConfig, Pull},
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::I2s,
    spi::master::{Config as SpiConfig, Spi},
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
    ledc::channel::Channel,
    interrupt::software::SoftwareInterrupt,
    time::Rate,
    timer::timg::{TimerGroup},
};

// I2C/SPI BUS SHARING IMPORTS
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::i2c::I2c as HalI2c;

// WIFI / METWORK IMPORTS

// DISPLAY IMPORTS
use display_interface_spi::SPIInterface;
use ili9341::{DisplaySize240x320, Ili9341, Orientation};

type DisplayType = Ili9341<
    SPIInterface<
        ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>,
        Output<'static>,
    >,
    Output<'static>,
>;



// YO-ESP == ESP32 -> I2S <- YO (BACKEND)
//      MICROPHONE ->BIDIR<- SPEAKER                      

struct VoiceHandler;

impl yo_esp::CommandHandler for VoiceHandler {    
    // 0x01 == WAKE WORD DETECTED
    fn on_detected(&mut self) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        alloc::boxed::Box::pin(async {
            // PLAY DING SOUND
            yo_esp::play_ding().await;
            // AND TURN ON DISPLAY
            crate::components::display::brightness_set("70");      
        })
    }

    // 0x02 == SERVER STARTED TRANSCRIPTION
    fn on_thinking(&mut self) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        Box::pin(async {
            // FLASH DISPLAY
            crate::components::display::brightness_set("0");
            Timer::after(Duration::from_millis(50)).await;
            crate::components::display::brightness_set("80");
            Timer::after(Duration::from_millis(50)).await;
            crate::components::display::brightness_set("0");
            Timer::after(Duration::from_millis(50)).await;
            crate::components::display::brightness_set("70");    
        })
    }

    // 0x03 == COMMAND EXECUTED
    fn on_executed(&mut self, _ms: Option<u64>) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        Box::pin(async move {       
            // PLAY DONE SOUND
            yo_esp::play_done().await;
            // AND TURN OFF DISPLAY
            crate::components::display::brightness_set("0");
        })
    }

    // 0x04 == FAILED COMMAND EXECUTION
    fn on_failed(&mut self, _ms: Option<u64>) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        Box::pin(async move {         
            // PLAY DUCK SAY `OH FUCK` SOUND
            yo_esp::play_fail().await;
           // AND TURN OFF DISPLAY
           crate::components::display::brightness_set("0");
        })
    }
}

// LOAD MODULES
mod components;
mod base;
mod apps;

// PANIC HANDLER
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// MEMORY
extern crate alloc;
use alloc::boxed::Box;

// BOOTLOADER (REQUIRED TO BOOT WITHOUT ESP-IDF)
esp_bootloader_esp_idf::esp_app_desc!();

// COMPILE-TIME ENVIORMENT VARIABLES
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");
const BACKEND_TCP_HOST: &str = env!("BACKEND_TCP_HOST");
const BACKEND_TCP_PORT_STR: &str = env!("BACKEND_TCP_PORT");
const FW_VERSION: &str = env!("CARGO_PKG_VERSION");
const SAMPLE_RATE: u32 = 16000;
const SAMPLE_COUNT: usize = 256;
const BUFFER_SIZE: usize = 4 * 4092;

// INIT ATOMIC DEFAULTS 
init_bool!(MIC_MUTED, false);
init_bool!(SPEAKER_MUTED, false);
init_bool!(DISPLAY_STATE, false);
init_u8!(MIC_VOLUME, 72);
init_u8!(SPEAKER_VOLUME, 58);
init_u8!(BACKLIGHT_PERCENT, 0);
init_u8!(BATTERY_PERCENT, 100);
init_u32!(BATTERY_VOLTAGE, 0);
init_u32!(CURRENT_IP, 0);
init_i32!(RSSI, 0);

pub static ES7210: CsMutex<RefCell<Option<es7210::Es7210>>> = CsMutex::new(RefCell::new(None));
pub static ES8311: CsMutex<RefCell<Option<es8311::Es8311>>> = CsMutex::new(RefCell::new(None));
pub static DISPLAY: CsMutex<RefCell<Option<DisplayType>>> = CsMutex::new(RefCell::new(None));
pub static I2C_BUS: CsMutex<RefCell<Option<I2cBus>>> = CsMutex::new(RefCell::new(None));
pub type I2cBus = I2c<'static, Blocking>;


// MONITOR AND CONTROL DISPLAY BRIGHTNESS
#[embassy_executor::task]
async fn backlight_task(channel: &'static mut Channel<'static, LowSpeed>) {
    loop {
        let percent = load!(BACKLIGHT_PERCENT);
        channel.set_duty(percent).unwrap();
        Timer::after(Duration::from_millis(100)).await;
    }
}


fn mic_volume_percent_to_db(percent: u8) -> i8 {
    let clamped = percent.clamp(0, 100) as i32;
    let db = -95 + (clamped * 127) / 100;
    db as i8
}

fn speaker_volume_percent(percent: u8) -> u8 {
    percent.clamp(0, 100)
}

// MONITOR AND EXECUTE AUDIO SETTINGS CHANGES
#[embassy_executor::task]
pub async fn audio_settings_task(i2c_bus: &'static CsMutex<RefCell<I2cBus>>) {
    let mut last_mic_vol = load!(MIC_VOLUME);
    let mut last_spk_vol = load!(SPEAKER_VOLUME);
    let mut last_mic_muted = load!(MIC_MUTED);
    let mut last_spk_muted = load!(SPEAKER_MUTED);

    loop {
        let mic_vol = load!(MIC_VOLUME);
        let spk_vol = load!(SPEAKER_VOLUME);
        let mic_muted = load!(MIC_MUTED);
        let spk_muted = load!(SPEAKER_MUTED);

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


// MAIN
#[allow(clippy::large_stack_frames)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // ALLOCATE MEMORY
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    // SOFTWARE INTERUPT SETUP
    let _sw_ints = esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    let sw_int0 = unsafe { SoftwareInterrupt::steal() }; 
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, sw_int0);
    info!("Started ESP32-S3-BOX-3 (version {})", FW_VERSION);
   

    // GPIO PINS
    let backlight = peripherals.GPIO47;
    let _touch_int = peripherals.GPIO3;

    let button_top_left = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up)
    );

    // ENABLE POWER AMPLIFIER
    // pa_enable.set_low() to mute
    let _pa_enable = Output::new(
        peripherals.GPIO46,
        Level::High,
        OutputConfig::default()
    );
    
    let occupancy = Input::new(
        peripherals.GPIO21,
        InputConfig::default().with_pull(Pull::Down)
    );

    // ADC / BATTERY
    let mut adc_config = AdcConfig::new();
    let battery_pin = peripherals.GPIO10;
    let mut adc_pin = adc_config.enable_pin(battery_pin, Attenuation::_0dB);
    let mut adc = Adc::new(peripherals.ADC1, adc_config);

    // I2C BUS A
    let i2c_a = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO18);    

    // I2C BUS B
    let i2c_b = I2c::new(
        peripherals.I2C1,
        I2cConfig::default().with_frequency(Rate::from_khz(50)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO41)
    .with_scl(peripherals.GPIO40);

    // LOCK & SHARE BUSSES
    let i2c_a_mutex = Box::leak(Box::new(CsMutex::new(RefCell::new(i2c_a))));
    let i2c_b_mutex = Box::leak(Box::new(CsMutex::new(RefCell::new(i2c_b))));

    // AUDIO CODEC CONFIGURATION
    let es7210 = es7210::Es7210::new(0x40);
    let es8311 = es8311::Es8311::new(0x18);

    { // CONFIGURE AUDIO CODECS
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
        let mut delay = Delay::new();
        match es8311.init(
            &mut i2c,
            &clock_cfg,
            es8311::Resolution::Bits16,
            es8311::Resolution::Bits16,
            &mut delay,
        ) {
            Ok(()) => info!("ES8311 initialised successfully"),
            Err(e) => info!("ES8311 init failed: {:?}", Debug2Format(&e)),
        }
        let _ = es8311.volume_set(&mut i2c, 80, None);
        let _ = es8311.mute(&mut i2c, false);
    } // RELEASE I2C

    
    // LEDC / BACKLIGHT
    let ledc = mk_static!(Ledc, Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    
    // LOW SPEED TIMER FOR 24 kHz WITH 10‑BIT DUTY RESOLUTION
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
    
    // CREATE A CHANNEL AND ASSIGN IT TO THE TIMER AND GPIO 47
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
    
    // LEAK THE CHANNEL TO GET STATIC MUT
    let backlight_channel: &'static mut _ = Box::leak(Box::new(channel0)); 
    
    // DISPLAY  
    let spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(40))
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO7)
    .with_mosi(peripherals.GPIO6)
    .with_miso(NoPin);
    
    let cs = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let rst = Output::new(peripherals.GPIO48, Level::Low, OutputConfig::default());
    
    let spi_dev = ExclusiveDevice::new(spi_bus, cs, Delay::new()).unwrap();
    let interface = SPIInterface::new(spi_dev, dc);
    let mut delay = Delay::new();
    
    let display = Ili9341::new(
        interface,
        rst,
        &mut delay,
        Orientation::Landscape,
        DisplaySize240x320,
    ).unwrap();
    
    critical_section::with(|cs| {
        DISPLAY.borrow_ref_mut(cs).replace(display);
    });

  
    // WIFI SETUP
    let backend_port: u16 = BACKEND_TCP_PORT_STR.parse().expect("Invalid BACKEND_TCP_PORT");
    let (stack, remote_addr) = base::wifi::init(&spawner, peripherals.WIFI, backend_port).await;

    // I2S AUDIO SETUP 
    let (_rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(BUFFER_SIZE);

    let i2s = I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        esp_hal::i2s::master::Config::new_tdm_philips()
            .with_signal_loopback(true)
            .with_sample_rate(Rate::from_hz(16000))
            .with_data_format(esp_hal::i2s::master::DataFormat::Data16Channel16)
            .with_endianness(esp_hal::i2s::master::Endianness::LittleEndian) 
            .with_channels(esp_hal::i2s::master::Channels::STEREO),            
    )
    .unwrap()
    .into_async()
    .with_mclk(peripherals.GPIO2);

    // AUDIO OUTPUT
    // BUILD I2S TX (MASTER) WITH BCLK, LRCLK AND DIGITAL OUT PINS 
    let i2s_tx = i2s.i2s_tx
        .with_bclk(peripherals.GPIO17)
        .with_ws(peripherals.GPIO45)
        .with_dout(peripherals.GPIO15)
        .build(tx_descriptors);

    // AUDIO INPUT
    // BUILD I2S RX (SLAVE) WITH DIGITAL-IN PIN 
    let i2s_rx = i2s
        .i2s_rx
        .with_din(peripherals.GPIO16)
        .build(rx_descriptors);

    // I2S TX CIRCULAR WRITE
    // CONTINUOSLY WRITE TO I2S TX TO KEEP CLOCKS UP FOR RX (SLAVE)
    let tx_transfer = match i2s_tx.write_dma_circular_async(tx_buffer) {
        Ok(t) => t,
        Err(e) => {
            error!("I2S circular TX failed: {:?}", Debug2Format(&e));
            panic!("I2S setup error");
        }
    };
    
    // YO-HANDLER 
    let handler: alloc::boxed::Box<dyn yo_esp::CommandHandler> = alloc::boxed::Box::new(VoiceHandler);  
    // INIT API ROUTES
    base::api::init_routes().await;


    // TASKS

    // WEB SERVER TASK PORT 80
    spawn!(spawner, tinyapi::web_server_task(stack));    
    // SPEAKER TASK
    spawn!(spawner, yo_esp::speaker_task(tx_transfer));
    // SPEAKER SERVER TASK (STREAM AUDIO TO THE SPEAKER OVER TCP PORT 12345)
    spawn!(spawner, yo_esp::stream_speaker(stack, backend_port));
    // MICROPHONE TASK (STREAM AUDIO TO SERVER OVER TCP PORT 12345)
    spawn!(spawner, yo_esp::audio_capture_task(i2s_rx, stack, remote_addr, "esp", handler));
    // AUDIO SETTINGS TASK
    //spawn!(spawner, audio_settings_task());
    // SENSOR TASK (TEMPERATURE/HUMIDITY)
    spawn!(spawner, components::aht20::sensor_task(i2c_b_mutex));
    // PRESENCE SENSOR TASK
    spawn!(spawner, components::presence::occupancy_task(occupancy));
    // BUTTON MONITOR TASK (TOP-LEFT BUTTON)
    spawn!(spawner, components::buttons::button_task(button_top_left));
    // DISPLAY TASK
    //spawn!(spawner, components::display::display_task());
    spawn!(spawner, backlight_task(backlight_channel));

    
    loop { // CALIBRATE BATTERY READ WITH PREDEFINED MIN/MAX mV VALUES
        let empty_battery = 1200 as u32;
        let full_battery = 1980 as u32;
        let empty_battery_charging = 4200 as u32;
        let full_battery_charging = 4521 as u32;

        // READ VOLTAGE
        let raw = adc.read_blocking(&mut adc_pin);
        let pin_voltage = raw as f32 * 1100.0 / 4095.0 / 1000.0;
        let battery_voltage = pin_voltage * 4.11;
        let voltage_mv = (battery_voltage * 1000.0) as u32;

        // CHARGING STATE DETECTION
        let charging = battery_voltage > 4.2;

        // BATTERY PERCENTAGE
        let (empty, full) = if charging {
            (empty_battery_charging, full_battery_charging)
        } else {
            (empty_battery, full_battery)
        };
        let raw_pct = (voltage_mv as i32 - empty as i32) * 100
                    / (full as i32 - empty as i32);
        let percentage = raw_pct.clamp(0, 100) as u8;

        // STORE AS MILLIVOLTS (u32) NO GO FLOAT     
        store!(BATTERY_VOLTAGE, voltage_mv);
        store!(BATTERY_PERCENT, percentage);

        // WIFI SIGNAL STRENGTH
        let rssi = load!(base::wifi::CURRENT_RSSI);
        store!(RSSI, rssi);

        // PRINT
        let emoji = match (percentage, charging) {
            (0..=10, false) => "🪫⚠️",
            (0..=10, true)  => "🪫⚡",
            (11..=29, false) => "🪫",
            (11..=29, true)  => "🪫⚡",
            (30..=70, false) => "🔋",
            (30..=70, true)  => "🔋⚡",
            (_, false)       => "🔋",
            (_, true)        => "🔋⚡",
        };        
        info!("{} {}% ({} mV)", emoji, percentage, voltage_mv);
        info!("🛜 {} dBm", rssi);
        // EVERY 60 SECONDS
        delay_s!(60);
    }
}
