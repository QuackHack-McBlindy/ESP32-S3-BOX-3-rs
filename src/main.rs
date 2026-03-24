#![no_std]
#![no_main]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]

use esp_println as _;
use defmt::{info, Debug2Format};

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::mutex::Mutex;

use esp_hal::{
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
    i2s::master::{I2s, I2sRx, Config as I2sConfig, DataFormat, Channels},
    i2s::master::asynch::I2sReadDmaTransferAsync,
    spi::master::{Config as SpiConfig, Spi},
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
    time::{Instant, Rate},
    timer::timg::TimerGroup,
};

// i2C/SPI bus sharing
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::i2c::I2c as HalI2c;

// WiFi
use esp_radio::wifi::{
    ClientConfig,
    ModeConfig,
    Config,
};

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
mod audio_input; // microphone
mod presence; // presence sensor
mod buttons; // physical buttons


#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// PSRAM?
extern crate alloc;
use alloc::boxed::Box;

// bootloader
esp_bootloader_esp_idf::esp_app_desc!();

// LOAD WIFI credentials (at compile-time)
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");

const SAMPLE_RATE: u32 = 16_000;
const BUFFER_SIZE: usize = 4096;
const SAMPLE_COUNT: usize = 256;
 
 
type DisplayType = Ili9341<
    SPIInterface<
        ExclusiveDevice<
            Spi<'static, Blocking>,
            Output<'static>,
            Delay,
        >,
        Output<'static>,
    >,
    Output<'static>,
>;


#[embassy_executor::task]
async fn display_task(mut display: DisplayType) -> ! {
    let style = MonoTextStyleBuilder::new()
        .font(&FONT_8X13)
        .text_color(Rgb565::WHITE)
        .background_color(Rgb565::BLACK)
        .build();

    let mut seconds_since_boot = 0u32;

    loop {
        let hours = seconds_since_boot / 3600;
        let minutes = (seconds_since_boot % 3600) / 60;
        let seconds = seconds_since_boot % 60;

        let time_string = alloc::format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

        display.clear(Rgb565::BLACK).unwrap();

        let text = Text::with_alignment(
            &time_string,
            Point::new(display.bounding_box().center().x, display.bounding_box().center().y),
            style,
            Alignment::Center,
        );
        text.draw(&mut display).unwrap();

        Timer::after(Duration::from_secs(1)).await;
        seconds_since_boot += 1;
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

    let button_mute = peripherals.GPIO46;

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
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    // low speed timer (Timer0) for 24 kHz with 10‑bit duty resolution
    let mut lstimer0 = ledc.timer::<LowSpeed>(esp_hal::ledc::timer::Number::Timer0);
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
            timer: &lstimer0,
            duty_pct: 0, // 0% brightness
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();
    
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
    let radio = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    // WIFI config
    let client_config = ClientConfig::default()
        .with_ssid(SSID.into())
        .with_password(PASSWORD.into());
    // wrap in ModeConfig
    let mode_config = ModeConfig::Client(client_config);

    // radio config
    let radio_config = Config::default();

    // init wifi controller
    let (mut wifi_controller, _interfaces) = esp_radio::wifi::new(
        &radio,
        peripherals.WIFI,
        radio_config,
    )
    .expect("Wi‑Fi - ❌ Failed to initialize Wi-Fi controller");

    // set operation mode
    wifi_controller
        .set_config(&mode_config)
        .expect("Wi‑Fi - ❌ Failed to set Wi‑Fi configuration");

    // start the wifi
    wifi_controller.start().expect("Failed to start Wi‑Fi");
    info!("Wi‑Fi - ⌛ connecting...");
    // connect
    match wifi_controller.connect_async().await {
        Ok(()) => { info!("Wi‑Fi - ✅ Connected successfully!"); }
        Err(e) => { info!("Wi‑Fi - ❌ Connection failed: {:?}", e); }
    }


    // I2S Audio setup 
    // DMA buffers
    let (rx_buffer, rx_descriptors, _, _) = dma_buffers!(BUFFER_SIZE);

    let i2s_config = I2sConfig::default()
        .with_sample_rate(Rate::from_hz(16_000))
        .with_data_format(DataFormat::Data16Channel16);

    let mut i2s = I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        i2s_config,
    ).unwrap()
    .with_mclk(i2s_mclk); 

    let mut i2s_rx = i2s.i2s_rx
        .with_bclk(i2s_bclk)
        .with_ws(i2s_lrclk)
        .with_din(i2s_din)
        .build(rx_descriptors);
      
        
    // TASKS
    let _ = spawner;

    // sensors
    spawner.spawn(aht20::sensor_task(i2c_b_mutex)).unwrap();
    // motion
    spawner.spawn(presence::occupancy_task(occupancy)).unwrap();
    // buttons
    spawner.spawn(buttons::top_left_button_task(button_top_left)).unwrap();
    // microphones
    //spawner.spawn(audio_input::audio_capture_task(i2s_rx)).unwrap();
    // display
    spawner.spawn(display_task(display)).unwrap();

    
    loop { // Calculate battery %
        let raw = adc.read_blocking(&mut adc_pin);
        let pin_voltage = raw as f32 * 1100.0 / 4095.0 / 1000.0;
        let battery_voltage = pin_voltage * 4.11;
        let percentage = ((battery_voltage - 3.0) / (4.2 - 3.0) * 100.0)
            .clamp(0.0, 100.0) as u8;

        info!("Battery: {}%,  ({=f32} V)", percentage, battery_voltage);
        Timer::after(Duration::from_secs(60)).await;
    }
}
