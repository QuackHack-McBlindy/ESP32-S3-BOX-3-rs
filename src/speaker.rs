use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::{Duration, Timer};
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use esp_hal::i2s::master::I2sTx;
use esp_hal::Async;
use defmt::{info, error};


pub const RING_BUFFER_SIZE: usize = 16384;
const DMA_BUFFER_SIZE: usize = 2048;


static PIPE: Pipe<CriticalSectionRawMutex, RING_BUFFER_SIZE> = Pipe::new();

const DING_SOUND: &[u8] = include_bytes!("./../sound/ding_esp.raw");
const DONE_SOUND: &[u8] = include_bytes!("./../sound/done_esp.wav");
const FAIL_SOUND: &[u8] = include_bytes!("./../sound/fail_esp.wav");

pub fn play(data: &[u8]) -> usize {
    PIPE.try_write(data).unwrap_or(0)
}


pub async fn play_sound(sound: &'static [u8]) {
    let mut offset = 0;
    while offset < sound.len() {
        let written = play(&sound[offset..]);
        if written == 0 {
            // PIPE FULL – wait and retry
            Timer::after(Duration::from_millis(1)).await;
        } else {
            offset += written;
        }
    }
}


pub async fn play_ding() {
    play_sound(DING_SOUND).await;
}

pub async fn play_done() {
    play_sound(DONE_SOUND).await;
}

pub async fn play_fail() {
    play_sound(FAIL_SOUND).await;
}

#[task]
pub async fn speaker_task(i2s_tx: &'static mut I2sTx<'static, Async>) -> ! {
    let mut dma_buffer = [0u8; DMA_BUFFER_SIZE];

    loop {
        let n = PIPE.read(&mut dma_buffer).await;
        if n > 0 {
            if let Err(e) = i2s_tx.write_dma_async(&mut dma_buffer[..n]).await {
                error!("I2S write error: {:?}", e);
            }
        } else { Timer::after(Duration::from_millis(10)).await; }
    }
}
