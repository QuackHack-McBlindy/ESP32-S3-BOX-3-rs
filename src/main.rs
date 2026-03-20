#![no_std]
#![no_main]
#![deny(clippy::mem_forget)]
#![deny(clippy::large_stack_frames)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Pull},
    i2c::master::{Config as I2cConfig, I2c},
    main,
    ledc::channel::ChannelIFace,
    ledc::timer::TimerIFace,
    ledc::{LSGlobalClkSource, Ledc, LowSpeed},
    time::{Instant, Rate},
    timer::timg::TimerGroup,
};

use esp_println as _;

use esp_radio::wifi::{
    ClientConfig,
    ModeConfig,
    Config,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// PSRAM?
extern crate alloc;

// load WiFi credentials from env vars at build time
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");

esp_bootloader_esp_idf::esp_app_desc!();



// TEMP/HUM SENSOR
async fn read_aht20_async(
    i2c: &mut I2c<'_, esp_hal::Blocking>,
) -> Option<(f32, f32)> {
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
async fn sensor_task(mut i2c: I2c<'static, esp_hal::Blocking>) {
    loop {
        if let Some((temp, hum)) = read_aht20_async(&mut i2c).await {
            info!("Temp: {=f32} °C, Hum: {=f32} %", temp, hum);
        } else { info!("AHT20 read failed"); }
        Timer::after(Duration::from_secs(10)).await;
    }
}

// MOTION SENSOR
#[embassy_executor::task]
async fn occupancy_task(mut occupancy: Input<'static>) {
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

// BUTTONS
#[embassy_executor::task]
async fn button_task(button: Input<'static>) {
    loop {
        if button.is_low() {
            info!("Top-Left Button pressed!");
            Timer::after(Duration::from_millis(200)).await;
            while button.is_low() { Timer::after(Duration::from_millis(10)).await; }
        }
        Timer::after(Duration::from_millis(50)).await;
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
    info!("Embassy initialized!");

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
    let mut i2c_bus_b = I2c::new(
        peripherals.I2C1,
        I2cConfig::default().with_frequency(Rate::from_khz(50)),
    )
    .unwrap()
    .with_sda(i2c_b_sda)
    .with_scl(i2c_b_scl);

    let es8311_addr = 0x18;

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

    // tasks
    let _ = spawner;

    spawner.spawn(sensor_task(i2c_bus_b)).unwrap();
    spawner.spawn(occupancy_task(occupancy)).unwrap();
    spawner.spawn(button_task(button_top_left)).unwrap();

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }    
}


