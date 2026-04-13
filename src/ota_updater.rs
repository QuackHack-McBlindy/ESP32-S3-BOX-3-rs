
#![no_std]
#![no_main]

use embedded_storage::Storage;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, InputConfig, Pull},
    main,
};
use esp_println::println;
use esp_storage::FlashStorage;

esp_bootloader_esp_idf::esp_app_desc!();

static OTA_IMAGE: &[u8] = include_bytes!("../../../target/ota_image");

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mut flash = FlashStorage::new(peripherals.FLASH);

    let mut buffer = [0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    let pt =
        esp_bootloader_esp_idf::partitions::read_partition_table(&mut flash, &mut buffer).unwrap();

    for part in pt.iter() {
        println!("{:?}", part);
    }

    println!("Currently booted partition {:?}", pt.booted_partition());

    let mut ota =
        esp_bootloader_esp_idf::ota_updater::OtaUpdater::new(&mut flash, &mut buffer).unwrap();

    let current = ota.selected_partition().unwrap();
    println!(
        "current image state {:?} (only relevant if the bootloader was built with auto-rollback support)",
        ota.current_ota_state()
    );
    println!("currently selected partition {:?}", current);

    if let Ok(state) = ota.current_ota_state() {
        if state == esp_bootloader_esp_idf::ota::OtaImageState::New
            || state == esp_bootloader_esp_idf::ota::OtaImageState::PendingVerify
        {
            println!("Changed state to VALID");
            ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::Valid)
                .unwrap();
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(any(feature = "esp32", feature = "esp32s2", feature = "esp32s3"))] {
            let button = peripherals.GPIO0;
        } else if #[cfg(any(feature = "esp32c5"))] {
            let button = peripherals.GPIO28;
        }else {
            let button = peripherals.GPIO9;
        }
    }

    let boot_button = Input::new(button, InputConfig::default().with_pull(Pull::Up));

    println!("Press boot button to flash and switch to the next OTA slot");
    let mut done = false;
    loop {
        if boot_button.is_low() && !done {
            done = true;

            let (mut next_app_partition, part_type) = ota.next_partition().unwrap();

            println!("Flashing image to {:?}", part_type);

            for (sector, chunk) in OTA_IMAGE.chunks(4096).enumerate() {
                println!("Writing sector {sector}...");

                next_app_partition
                    .write((sector * 4096) as u32, chunk)
                    .unwrap();
            }

            println!("Changing OTA slot and setting the state to NEW");

            ota.activate_next_partition().unwrap();
            ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::New)
                .unwrap();
        }
    }
}
