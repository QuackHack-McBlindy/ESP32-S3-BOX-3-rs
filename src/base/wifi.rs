use core::sync::atomic::{AtomicI32, Ordering};
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_time::Timer;
use esp_radio::wifi::{
    ClientConfig, Config as WifiConfig, ModeConfig, PowerSaveMode, WifiController, WifiDevice,
    WifiEvent, WifiStaState,
};
use defmt::info;

// global RSSI value
pub static CURRENT_RSSI: AtomicI32 = AtomicI32::new(0);

#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    // power saving
    if let Err(e) = controller.set_power_saving(PowerSaveMode::Maximum) {
        info!("Failed to set power saving: {:?}", e);
    }

    loop {
        if let WifiStaState::Connected = esp_radio::wifi::sta_state() {
            // update RSSI periodically while connected
            if let Ok(rssi) = controller.rssi() {
                CURRENT_RSSI.store(rssi, Ordering::Relaxed);
            }

            select(
                controller.wait_for_event(WifiEvent::StaDisconnected),
                Timer::after(embassy_time::Duration::from_millis(6000)),
            )
            .await;
        }

        // not started - start 
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = ClientConfig::default()
                .with_ssid(crate::SSID.into())
                .with_password(crate::PASSWORD.into());
            let mode_config = ModeConfig::Client(client_config);
            controller.set_config(&mode_config).unwrap();

            if let Err(e) = controller.start_async().await {
                info!("Failed to start WiFi: {:?}", e);
                Timer::after(embassy_time::Duration::from_millis(5000)).await;
                continue;
            }
        }

        match controller.connect_async().await {
            Ok(()) => info!("WiFi - ✅ connected!"),
            Err(e) => {
                info!("WiFi - ❌ connection failed: {:?}", e);
                Timer::after(embassy_time::Duration::from_millis(5000)).await;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn net_task(mut runner: embassy_net::Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

pub async fn sleep(millis: u64) {
    Timer::after(embassy_time::Duration::from_millis(millis)).await;
}
