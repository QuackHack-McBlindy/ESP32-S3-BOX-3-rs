use embassy_executor::task;
use embassy_time::{Timer, Duration};
use esp_hal::gpio::Input;
use crate::speaker;

#[task]
pub async fn top_left_button_task(button: Input<'static>) {

    loop {
        if button.is_low() {
            defmt::info!("Top-Left Button pressed!");
            speaker::play_ding().await;

            // wait until button is released
            Timer::after(Duration::from_millis(200)).await;
            while button.is_low() {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
