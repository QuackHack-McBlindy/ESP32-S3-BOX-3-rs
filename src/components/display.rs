// COMPONENTS/DISPLAY
use defmt::info;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use alloc::string::String;
use crate::BACKLIGHT_PERCENT;


pub fn brightness_set(value: &str) {
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        BACKLIGHT_PERCENT.store(percent, Ordering::Relaxed);
        info!("🔆 {}%", percent);
    } else {
        info!("invalid brightness value!");
    }
}
