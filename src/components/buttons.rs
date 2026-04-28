// COMPONENTS/BUTTONS
use embassy_executor::task;
use embassy_time::{Timer, Duration};
use esp_hal::gpio::Input;
use crate::init_bool;
use crate::store;
use crate::toggle;

init_bool!(BUTTON_PRESSED, false);

// TASK THAT
#[task] // TAKES BUTTON AS ARGUMENT
pub async fn button_task(button: Input<'static>) {
    loop { // MONITOR BUTTON
        // LOW BUTTON == PRESSED BUTTON
        if button.is_low() { 
            // SET THE ATOMIC BOOL FLAG
            store!(BUTTON_PRESSED, true);
            // TOGGLE DISPLAY
            toggle!(crate::DISPLAY_STATE);
            yo_esp::play_ding().await;

            // WAIT FOR RELEASE
            Timer::after(Duration::from_millis(200)).await;
            while button.is_low() {
                Timer::after(Duration::from_millis(10)).await;
            }
        } else { store!(BUTTON_PRESSED, false); }
        Timer::after(Duration::from_millis(50)).await;
    }
}

// `swap(false)` AUTOMATICALLY READS THE CURRENT VALUE AND SETS IT TO FALSE
// BUTTON WAS PRESSED SINCE LAST CHECK
// if BUTTON_PRESSED.swap(false, core::sync::atomic::Ordering::AcqRel) { crate::components::speaker::play_ding().await; }
