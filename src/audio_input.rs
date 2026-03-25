use defmt::info;
use embassy_executor::task;
use esp_hal::i2s::master::{
    I2sRx,
    asynch::I2sReadDmaTransferAsync,
};
use esp_hal::Async;

const SAMPLE_COUNT: usize = 256;
const BUFFER_SIZE: usize = SAMPLE_COUNT * 2;

#[task]
pub async fn audio_capture_task(
    mut i2s_rx: I2sRx<'static, Async>
) {
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        match i2s_rx.read_dma_async(&mut buffer).await {
            Ok(()) => {
                let samples: &[u16] = unsafe {
                    core::slice::from_raw_parts(
                        buffer.as_ptr() as *const u16,
                        SAMPLE_COUNT,
                    )
                };

                let first = samples[0];
                let second = samples[1];

                info!("Audio: {} {}", first, second);
            }
            Err(e) => { info!("I2S read error: {:?}", e); }
        }
    }
}
