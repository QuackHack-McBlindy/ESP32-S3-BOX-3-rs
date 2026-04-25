// COMPONENTS/PRESENCE
// RADAR SEBSOR  (MS58-3909S68U4)
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use defmt::debug;

pub static PRESENCE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// SIMPLE TASK THAT MONITORS HIGH/LOW SIGNALS
// FROM THE PIN AND STORES VALUE AS ATOMIC
#[task]
pub async fn occupancy_task(occupancy: Input<'static>) {
    let mut last = occupancy.is_high();
    loop { // HIGH == YOU ARE DETECTED BY RADAR
        let current = occupancy.is_high();
        
        // STORE AS ATOMIC
        PRESENCE.store(current, core::sync::atomic::Ordering::Relaxed);
        
        // LOG
        if current != last {
            if current { 
                debug!("Motion!");
            } else { 
                debug!("No motion.");
            }
            last = current;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
