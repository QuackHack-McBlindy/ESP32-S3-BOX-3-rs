use embassy_executor::task;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use defmt::info;

#[task]
pub async fn occupancy_task(occupancy: Input<'static>) {
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
