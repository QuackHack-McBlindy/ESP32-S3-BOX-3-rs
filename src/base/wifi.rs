// BASE/WIFI
// BASIC WIFI CONFIGURATION
// ++ EMBASSY-NET RUNNER
use core::sync::atomic::{AtomicI32, Ordering};
use embassy_futures::select::{select, Either};
use embassy_time::{Timer, Duration};
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{Config, PowerSaveMode, WifiController, Interface};
use defmt::info;

use crate::alloc::string::ToString;

pub static CURRENT_RSSI: AtomicI32 = AtomicI32::new(0);

// WIFI CONNECTION TASK
#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    let station_config = esp_radio::wifi::sta::StationConfig::default()
        .with_ssid(crate::SSID)
        .with_password(crate::PASSWORD.to_string());

    let wifi_config = esp_radio::wifi::Config::Station(station_config);

    controller.set_config(&wifi_config).unwrap();

    // ENABLE POWER SAVING
    if let Err(e) = controller.set_power_saving(PowerSaveMode::Maximum) {
        info!("failed to set power saving: {:?}", e);
    }

    loop {
        match controller.connect_async().await {
            Ok(conn_info) => {
                info!(
                    "WiFi - ✅ connected, channel: {}",
                    conn_info.channel
                );

                // LOOP TO UPDATE & STORE RSSI ATOMIC
                loop {
                    if let Ok(rssi) = controller.rssi() {
                        CURRENT_RSSI.store(rssi, core::sync::atomic::Ordering::Relaxed);
                    }

                    match select(
                        controller.wait_for_disconnect_async(),
                        Timer::after(Duration::from_millis(6000)),
                    )
                    .await
                    {
                        Either::First(result) => {
                            match result {
                                Ok(info) => info!(
                                    "WiFi - ❌ disconnected! reason: {:?}",
                                    info.reason
                                ),
                                Err(e) => info!("WiFi - ❌ disconnect ERROR: {:?}", e),
                            }
                            break; // EXIT INNER LOOP TO RECONNECT
                        }
                        Either::Second(()) => {
                            // TIMEOUT – LOOP AGAIN
                        }
                    }
                }
            }
            Err(e) => {
                info!("WiFi - ❌ connection failed: {:?}", e);
                Timer::after(Duration::from_millis(5000)).await;
            }
        }
    }
}

// EMBASSY-NET RUNNER
#[embassy_executor::task]
pub async fn net_task(mut runner: embassy_net::Runner<'static, Interface<'static>>) {
    runner.run().await;
}

// PUB SLEEP FUNCTION
pub async fn sleep(millis: u64) {
    Timer::after(Duration::from_millis(millis)).await;
}
