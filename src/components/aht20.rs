// COMPONENTS/AHT20
// READS TEMPERATURE AND HUMIDITY DATA
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use defmt::info;
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use embedded_hal::i2c::I2c as HalI2c;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::i2c::master::I2c;
use esp_hal::Blocking;
use crate::{init_u8, store};

init_u8!(HUMIDITY, 0);
init_u8!(TEMPERATURE, 0);

pub async fn read_aht20_async<I2C: HalI2c>(i2c: &mut I2C) -> Option<(f32, f32)> {
    // SEND INITIALIZATION COMMAND TO THE AHT20 SENSOR (0xBE 0x08 0x00)
    // THIS IS THE "INIT" COMMAND TO PREPARE THE SENSOR FOR MEASUREMENT
    let init_cmd = [0xBE, 0x08, 0x00];
    i2c.write(0x38, &init_cmd).ok()?;
    Timer::after(Duration::from_millis(10)).await;

    // SEND MEASUREMENT TRIGGER COMMAND TO THE AHT20 SENSOR (0xAC 0x33 0x00)
    // THIS COMMAND STARTS A TEMPERATURE & HUMIDITY MEASUREMENT CYCLE
    let measure_cmd = [0xAC, 0x33, 0x00];
    i2c.write(0x38, &measure_cmd).ok()?;
    Timer::after(Duration::from_millis(80)).await;

    // READ 6 BYTES OF MEASUREMENT DATA FROM THE AHT20 SENSOR
    // BYTE 0 == STATUS / SENSOR STATE
    // BYTES 1-2 & PART OF BYTE 3 == 20-BIT RAW HUMIDITY DATA
    // BYTES 3 (REMAINING) TO 5 == 20-BIT RAW TEMPERATURE DATA
    let mut buf = [0u8; 6];
    i2c.read(0x38, &mut buf).ok()?;

    // CHECK STATUS BIT IN BYTE 0 (BIT 7) == IF SET MEASUREMENT IS NOT READY (BUSY OR FAILED)
    // RETURN NONE TO INDICATE A READ FAILURE
    if buf[0] & 0x80 != 0 {
        return None;
    }

    // CONSTRUCT 20-BIT RAW HUMIDITY VALUE FROM DATA BYTES
    // BUF[1] == H[19:12], BUF[2] == H[11:4], BUF[3][7:4] == H[3:0]
    // RAW_HUM == (BUF[1] << 12) | (BUF[2] << 4) | (BUF[3] >> 4)
    let raw_hum = ((buf[1] as u32) << 12) | ((buf[2] as u32) << 4) | ((buf[3] as u32) >> 4);

    // CONSTRUCT 20-BIT RAW TEMPERATURE VALUE FROM DATA BYTES
    // BUF[3][3:0] == T[19:16], BUF[4] == T[15:8], BUF[5] == T[7:0]
    // RAW_TEMP == ((BUF[3] & 0x0F) << 16) | (BUF[4] << 8) | BUF[5]
    let raw_temp = (((buf[3] as u32) & 0x0F) << 16)
        | ((buf[4] as u32) << 8)
        | (buf[5] as u32);

    // CONVERT RAW HUMIDITY TO PERCENT RELATIVE HUMIDITY
    // HUMIDITY (%) == (RAW_HUM / 2^20) * 100
    let humidity = (raw_hum as f32) * 100.0 / (1 << 20) as f32;

    // CONVERT RAW TEMPERATURE TO DEGREES CELSIUS
    // TEMPERATURE (°C) == (RAW_TEMP / 2^20) * 200 - 50
    let temperature = (raw_temp as f32) * 200.0 / (1 << 20) as f32 - 50.0;

    Some((temperature, humidity))
}


// TASK THAT READS AND LOGS EVERY 60 SECONDS
#[task]
pub async fn sensor_task(i2c_mutex: &'static CsMutex<RefCell<I2c<'static, Blocking>>>) {
    loop {
        let mut i2c = CriticalSectionDevice::new(i2c_mutex);
        if let Some((temp, hum)) = read_aht20_async(&mut i2c).await {
            // SCALE TEMPERATURE AND HUMIDITY BY 10 FOR DISPLAY WITH ONE DECIMAL PLACE
            let temp_int = (temp * 10.0) as u16;
            let hum_int = (hum * 10.0) as u16;

            // STORE AS ATOMIC
            store!(TEMPERATURE, temp_int as u8);
            store!(HUMIDITY, hum_int as u8);
            
            // PRINT
            info!("🌡️ {=u16}.{=u16} °C, 💨 {=u16}.{=u16}%", temp_int / 10, temp_int % 10, hum_int / 10, hum_int % 10);
            tinyapi::log!("🌡️ {}.{} °C, 💨 {}.{}%", temp_whole, temp_frac, hum_whole, hum_frac);
        } else { info!("AHT20 read failed"); }
        Timer::after(Duration::from_secs(60)).await;
    }
}
