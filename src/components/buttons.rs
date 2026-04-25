// COMPONENTS/BUTTONS
use embassy_executor::task;
use embassy_time::{Timer, Duration};
use esp_hal::gpio::Input;

pub static BUTTON_PRESSED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// TASK THAT
#[task] // TAKES BUTTON AS ARGUMENT
pub async fn button_task(button: Input<'static>) {
    loop { // MONITOR BUTTON
        // LOW BUTTON == PRESSED BUTTON
        if button.is_low() { 
            // SET THE ATOMIC BOOL FLAG
            BUTTON_PRESSED.store(true, core::sync::atomic::Ordering::Relaxed);

            // WAIT UNTIL BUTTON IS RELEASED
            Timer::after(Duration::from_millis(200)).await;
            while button.is_low() {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}

// `swap(false)` AUTOMATICALLY READS THE CURRENT VALUE AND SETS IT TO FALSE
// BUTTON WAS PRESSED SINCE LAST CHECK
// if BUTTON_PRESSED.swap(false, core::sync::atomic::Ordering::AcqRel) { crate::components::speaker::play_ding().await; }
