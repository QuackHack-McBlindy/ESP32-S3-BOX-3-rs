use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use defmt::info;
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use embedded_hal::i2c::I2c as HalI2c;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::i2c::master::I2c;
use esp_hal::Blocking;


pub async fn read_aht20_async<I2C: HalI2c>(i2c: &mut I2C) -> Option<(f32, f32)> {
    // init with soft reset)
    let init_cmd = [0xBE, 0x08, 0x00];
    i2c.write(0x38, &init_cmd).ok()?;
    Timer::after(Duration::from_millis(10)).await;

    // begin measurement
    let measure_cmd = [0xAC, 0x33, 0x00];
    i2c.write(0x38, &measure_cmd).ok()?;
    Timer::after(Duration::from_millis(80)).await;

    // read 6 bytes
    let mut buf = [0u8; 6];
    i2c.read(0x38, &mut buf).ok()?;

    // validate measurement (bit 7 of status should be 0)
    if buf[0] & 0x80 != 0 {
        return None;
    }

    // combine bytes into 20-bit raw humidity value
    let raw_hum = ((buf[1] as u32) << 12) | ((buf[2] as u32) << 4) | ((buf[3] as u32) >> 4);
    // combine bytes into 20-bit raw temperature
    let raw_temp = (((buf[3] as u32) & 0x0F) << 16)
        | ((buf[4] as u32) << 8)
        | (buf[5] as u32);
    
    // convert raw humidity to percentage (0-100%)
    let humidity = (raw_hum as f32) * 100.0 / (1 << 20) as f32;
    // convert raw temperature to Celsius (-50 to +150°C)
    let temperature = (raw_temp as f32) * 200.0 / (1 << 20) as f32 - 50.0;

    Some((temperature, humidity))
}


#[task]
pub async fn sensor_task(i2c_mutex: &'static CsMutex<RefCell<I2c<'static, Blocking>>>) {
    loop {
        let mut i2c = CriticalSectionDevice::new(i2c_mutex);
        if let Some((temp, hum)) = read_aht20_async(&mut i2c).await {
            info!("Temp: {=f32} °C, Hum: {=f32} %", temp, hum);
        } else {
            info!("AHT20 read failed");
        }
        Timer::after(Duration::from_secs(60)).await;
    }
}
