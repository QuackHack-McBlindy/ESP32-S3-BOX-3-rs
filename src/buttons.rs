use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
use defmt::info;
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::gpio::Input;
use esp_hal::i2c::master::I2c;
use esp_hal::Blocking;


#[task]
pub async fn top_left_button_task(button: Input<'static>) {
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
