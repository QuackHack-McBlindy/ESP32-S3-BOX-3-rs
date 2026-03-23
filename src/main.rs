#![no_std]
#![no_main]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]

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
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Pull},
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{I2s, I2sRx, Config as I2sConfig, DataFormat, Channels},
    i2s::master::asynch::I2sReadDmaTransferAsync,
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
    time::{Instant, Rate},
    timer::timg::TimerGroup,
};

// i2C bus sharing
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use embedded_hal::i2c::I2c as HalI2c;


use esp_println as _;

// WiFi
use esp_radio::wifi::{
    ClientConfig,
    ModeConfig,
    Config,
};

// load modules
mod es7210; // microphone audio codec
mod es8311; // speaker audio codec
// mod aht20; // temperature & humidity sensor


#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// PSRAM?
extern crate alloc;
use alloc::boxed::Box;

// load WiFi credentials from env vars at build time
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");

const SAMPLE_RATE: u32 = 16_000;
const BUFFER_SIZE: usize = 4096;
const SAMPLE_COUNT: usize = 256;
  

// bootloader
esp_bootloader_esp_idf::esp_app_desc!();


// TEMP/HUM SENSOR
async fn read_aht20_async<I2C: HalI2c>(i2c: &mut I2C) -> Option<(f32, f32)> {
    let init_cmd = [0xBE, 0x08, 0x00];
    i2c.write(0x38, &init_cmd).ok()?;
    Timer::after(Duration::from_millis(10)).await;

    let measure_cmd = [0xAC, 0x33, 0x00];
    i2c.write(0x38, &measure_cmd).ok()?;
    Timer::after(Duration::from_millis(80)).await;

    let mut buf = [0u8; 6];
    i2c.read(0x38, &mut buf).ok()?;

    if buf[0] & 0x80 != 0 {
        return None;
    }

    let raw_hum = ((buf[1] as u32) << 12) | ((buf[2] as u32) << 4) | ((buf[3] as u32) >> 4);
    let raw_temp = (((buf[3] as u32) & 0x0F) << 16)
        | ((buf[4] as u32) << 8)
        | (buf[5] as u32);

    let humidity = (raw_hum as f32) * 100.0 / (1 << 20) as f32;
    let temperature = (raw_temp as f32) * 200.0 / (1 << 20) as f32 - 50.0;

    Some((temperature, humidity))
}

#[embassy_executor::task]
async fn sensor_task(i2c_mutex: &'static CsMutex<RefCell<I2c<'static, esp_hal::Blocking>>>) {
    loop {
        let mut i2c = CriticalSectionDevice::new(i2c_mutex);
        if let Some((temp, hum)) = read_aht20_async(&mut i2c).await {
            info!("Temp: {=f32} °C, Hum: {=f32} %", temp, hum);
        } else {
            info!("AHT20 read failed");
        } // i2c dropped
        
        Timer::after(Duration::from_secs(10)).await;
    }
}

#[embassy_executor::task]
async fn occupancy_task(occupancy: Input<'static>) {
    let mut last = occupancy.is_high();
    loop {
        let current = occupancy.is_high();
        if current != last {
            if current {
                info!("Motion!");
            } else {
                info!("No motion.");
            }
            last = current;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}

#[embassy_executor::task]
async fn button_task(button: Input<'static>) {
    loop {
        if button.is_low() {
            info!("Top-Left Button pressed!");
            Timer::after(Duration::from_millis(200)).await;
            while button.is_low() {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}


#[embassy_executor::task]
async fn audio_capture_task(mut i2s_rx: I2sRx<'static, Blocking>) {
    let mut samples = [0u16; SAMPLE_COUNT];

    loop {
        if let Err(e) = i2s_rx.read_words(&mut samples) {
            info!("I2S read error: {:?}", e);
            continue;
        }

        let raw_bytes = unsafe {
            core::slice::from_raw_parts(
                samples.as_ptr().cast::<u8>(),
                core::mem::size_of_val(&samples),
            )
        };
        let first_four = &raw_bytes[..4];
        info!("Audio: {:02X} {:02X} {:02X} {:02X} ...",
            first_four[0], first_four[1], first_four[2], first_four[3]);
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

    //////////////////////////////////////////////////
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

    let battery_adc = peripherals.GPIO10;

    ////////////////////////////////////
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
        if let Err(e) = es7210.config_volume(&mut i2c, 20) {
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
        let _ = es8311.voice_volume_set(&mut i2c, 80, None);
        let _ = es8311.voice_mute(&mut i2c, false);
    } // release i2c
    


    /////////////////////////////////////////
    // LEDC
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
            duty_pct: 10, // 10%
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();


    ////////////////////////
    // WIFI
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


//////////////////////////////
    // I2S
//////////////////////////////
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
      
         
    // let mut dac_stream = i2s.i2s_tx
    //    .with_bclk(i2s_bclk)
    //    .with_ws(i2s_lrclk)
    //    .with_dout(i2s_dout)
    //    .build();

    // PLAY BOOT SOUND
    // let samples: [i16; 1000] = core::array::from_fn(|i| {
    //    let t = i as f32 / 16000.0;
    //    (32767.0 * (2.0 * core::f32::consts::PI * 440.0 * t).sin()) as i16
    // });

    // dac_stream.write(&samples).await;
//////////////////////////////////////
    // tasks
    let _ = spawner;

    // MONITOR
    // sensors
    spawner.spawn(sensor_task(i2c_b_mutex)).unwrap();
    // motion
    spawner.spawn(occupancy_task(occupancy)).unwrap();
    // buttons
    spawner.spawn(button_task(button_top_left)).unwrap();
    // microphones
    //spawner.spawn(audio_capture_task(i2s_rx)).unwrap();
    
    loop {
        Timer::after(Duration::from_secs(60)).await;
    }    
}
