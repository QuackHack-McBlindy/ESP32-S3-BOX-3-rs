use core::sync::atomic::{AtomicI32, Ordering};
use embassy_futures::select::{select, Either};
use embassy_time::{Timer, Duration};
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{Config, PowerSaveMode, WifiController, Interface};
use defmt::info;
use crate::SSID;
use crate::PASSWORD;
pub static CURRENT_RSSI: AtomicI32 = AtomicI32::new(0);
use crate::alloc::string::ToString;

#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    let station_config = esp_radio::wifi::sta::StationConfig::default()
        .with_ssid(crate::SSID)
        .with_password(crate::PASSWORD.to_string());

    let wifi_config = esp_radio::wifi::Config::Station(station_config);

    controller.set_config(&wifi_config).unwrap();

    // enable power saving
    if let Err(e) = controller.set_power_saving(PowerSaveMode::Maximum) {
        info!("Failed to set power saving: {:?}", e);
    }

    loop {
        match controller.connect_async().await {
            Ok(conn_info) => {
                info!(
                    "WiFi - ✅ connected, channel: {}",
                    conn_info.channel
                );

                loop {
                    if let Ok(rssi) = controller.rssi() {
                        CURRENT_RSSI.store(rssi, Ordering::Relaxed);
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
                                    "WiFi - ❌ disconnected, reason: {:?}",
                                    info.reason
                                ),
                                Err(e) => info!("WiFi - ❌ disconnect error: {:?}", e),
                            }
                            break; // exit inner loop to reconnect
                        }
                        Either::Second(()) => {
                            // timeout – loop again
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

#[embassy_executor::task]
pub async fn net_task(mut runner: embassy_net::Runner<'static, Interface<'static>>) {
    runner.run().await;
}

pub async fn sleep(millis: u64) {
    Timer::after(Duration::from_millis(millis)).await;
}
