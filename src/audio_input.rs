use defmt::info;
use embassy_executor::task;
use embassy_time::Duration;
use esp_hal::i2s::master::I2sRx;
use esp_hal::Blocking;

const SAMPLE_COUNT: usize = 256;

#[task]
pub async fn audio_capture_task(mut i2s_rx: I2sRx<'static, Blocking>) {
    let mut samples = [0u16; SAMPLE_COUNT];

    loop {
        if let Err(e) = i2s_rx.read_words(&mut samples) {
            info!("I2S read error: {:?}", e);
            continue;
        }

        let raw_bytes = unsafe {
            core::slice::from_raw_parts(
                samples.as_ptr().cast::<u8>(),
                core::mem::size_of_val(&samples),
            )
        };
        let first_four = &raw_bytes[..4];
        info!("Audio: {:02X} {:02X} {:02X} {:02X} ...",
            first_four[0], first_four[1], first_four[2], first_four[3]);
    }
}
