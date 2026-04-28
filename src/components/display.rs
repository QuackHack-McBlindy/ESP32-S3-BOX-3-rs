// COMPONENTS/DISPLAY
use defmt::info;
use alloc::format;
use alloc::string::ToString;
use embassy_time::Duration;
use crate::BACKLIGHT_PERCENT;
use crate::load;
use crate::store;
use crate::wait_ms;
use crate::DISPLAY;
use crate::RSSI;
use crate::CURRENT_IP;
use crate::BATTERY_PERCENT;


#[embassy_executor::task]
async fn display_task() {
    use embedded_graphics::{
        geometry::Point,
        mono_font::{MonoTextStyle, ascii::FONT_8X13},
        text::{Text, Alignment},
        pixelcolor::Rgb565,
        prelude::*,
    };

    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    let mut last_battery = 255;
    let mut last_ip = 0u32;
    let mut last_rssi = 0;

    loop {
        let battery = load!(BATTERY_PERCENT);
        let ip_raw = load!(CURRENT_IP);
        let rssi = load!(RSSI);

        let ip_str = if ip_raw == 0 {
            "IP: none".to_string()
        } else {
            let ip = embassy_net::Ipv4Address::from(ip_raw);
            format!("IP: {}", ip)
        };

        if battery != last_battery || ip_raw != last_ip || rssi != last_rssi {
            // RE-DRAW WHEN SOMETHING CHANGES
            critical_section::with(|cs| {
                if let Some(display) = DISPLAY.borrow_ref_mut(cs).as_mut() {
                    // CLEAR SCREEN (BLACK)
                    display.clear(Rgb565::BLACK).unwrap();
                    // DRAW BATTERY
                    let battery_text = format!("🔋 {}%", battery);
                    Text::with_alignment(
                        &battery_text,
                        Point::new(120, 20),
                        style,
                        Alignment::Center,
                    )
                    .draw(display)
                    .unwrap();

                    // DRAW IP
                    Text::with_alignment(
                        &ip_str,
                        Point::new(120, 50),
                        style,
                        Alignment::Center,
                    )
                    .draw(display)
                    .unwrap();

                    // DRAW RSSI
                    let rssi_text = format!("📶 {} dBm", rssi);
                    Text::with_alignment(
                        &rssi_text,
                        Point::new(120, 80),
                        style,
                        Alignment::Center,
                    )
                    .draw(display)
                    .unwrap();
                }
            });

            last_battery = battery;
            last_ip = ip_raw;
            last_rssi = rssi;
        }
        embassy_time::Timer::after(Duration::from_millis(500)).await;
    }
}

// BRIGHTNESS_SET
pub fn brightness_set(value: &str) {
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        store!(crate::BACKLIGHT_PERCENT, percent);
        info!("🔆 {}%", percent);
    } else { info!("invalid brightness value!"); }
}

// FLASH DISPLAY
pub fn flash(times: u32) {
    let original = load!(BACKLIGHT_PERCENT);
    for _ in 0..times {
        store!(crate::BACKLIGHT_PERCENT, 80);
        wait_ms!(200);
        store!(crate::BACKLIGHT_PERCENT, 0);
        wait_ms!(200);
    }
    store!(BACKLIGHT_PERCENT, original);
}
