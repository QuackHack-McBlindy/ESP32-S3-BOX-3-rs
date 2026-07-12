// COMPONENTS/PRESENCE
// RADAR SEBSOR  (MS58-3909S68U4)

use embassy_executor::task;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;

// SIMPLE TASK THAT MONITORS HIGH/LOW SIGNALS
// FROM THE PIN AND STORES VALUE AS ATOMIC
#[task]
pub async fn occupancy_task(occupancy: Input<'static>) {
    let mut last = occupancy.is_high();
    loop { // HIGH == YOU ARE DETECTED BY RADAR
        let current = occupancy.is_high();
        
        // STORE AS ATOMIC
        crate::store!(crate::state::PRESENCE, current);
        // LOG
        if current != last {
            if current { 
                defmt::debug!("Motion!");
            } else { 
                defmt::debug!("No motion.");
            }
            last = current;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
