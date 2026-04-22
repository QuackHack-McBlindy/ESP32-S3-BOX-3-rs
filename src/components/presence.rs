use embassy_executor::task;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use defmt::debug;
use core::sync::atomic::{AtomicBool, Ordering};

pub static PRESENCE: AtomicBool = AtomicBool::new(false);

#[task]
pub async fn occupancy_task(occupancy: Input<'static>) {
    let mut last = occupancy.is_high();
    loop {
        let current = occupancy.is_high();
        PRESENCE.store(current, Ordering::Relaxed);
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
