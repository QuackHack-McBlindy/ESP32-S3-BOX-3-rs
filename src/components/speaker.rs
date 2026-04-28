// COMPONENTS/SPEAKER
// PROVIDES A SPEAKER AND SOME SIMPLE SOUNDS STORED AS BYTES
// ++ TCP SERVER TASK THAT RECEIEVES AUDIO DATA AND PLAYS IT ON SPEAKER  
use esp_hal::Async;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::{Duration, Timer};
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use esp_hal::i2s::master::I2sTx;
use defmt::{info, error, Debug2Format};
use alloc::vec;
use alloc::vec::Vec;
use embassy_futures::select::{select, Either};

// SOUND FILES
const DING_SOUND: &[u8] = include_bytes!("./../../assets/sound/ding_esp.raw");
const DONE_SOUND: &[u8] = include_bytes!("./../../assets/sound/done_esp.wav");
const FAIL_SOUND: &[u8] = include_bytes!("./../../assets/sound/fail_esp.wav");


const DMA_BUFFER_SIZE: usize = crate::BUFFER_SIZE;
const STEREO_SAMPLES_PER_WRITE: usize = 256;
const MONO_SAMPLES_PER_WRITE: usize = STEREO_SAMPLES_PER_WRITE / 2;
const OWW_MODEL_CHUNK_SIZE: usize = 1280;
const DMA_SAMPLES: usize = 256;
const DMA_BUFFER_BYTES: usize = DMA_SAMPLES * 4;
const PLAYBACK_TCP_RX_BUF_SIZE: usize = 4096;
const PLAYBACK_TCP_TX_BUF_SIZE: usize = 2048;
pub const RING_BUFFER_SIZE: usize = 16384;

// A PIPE TO PUSH AUDIO DATA THROUGH
static PIPE: Pipe<CriticalSectionRawMutex, RING_BUFFER_SIZE> = Pipe::new();

// FUNCTION TO WRITE DATA INTO PIPE
pub fn play(data: &[u8]) -> usize { PIPE.try_write(data).unwrap_or(0) }



// FUNCTION TO PLAY A STORED SOUND
pub async fn play_sound(sound: &'static [u8]) {
    let mut offset = 0;
    while offset < sound.len() {
        let written = play(&sound[offset..]);
        if written == 0 {
            embassy_time::Timer::after(embassy_time::Duration::from_millis(1)).await;
        } else {
            offset += written;
        }
    }
}

// FUNCTION TO PLAY DING SOUND
pub async fn play_ding() { play_sound(DING_SOUND).await; }
// FUNCTION TO PLAY DONE SOUND
pub async fn play_done() { play_sound(DONE_SOUND).await; }
// FUNCTION TO PLAY FAIL SOUND
pub async fn play_fail() { play_sound(FAIL_SOUND).await; }


// TASK THAT RUNS SPEAKER
#[task]
pub async fn speaker_task(
    mut transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_BUFFER_SIZE]>
) -> ! {
    let mut pipe_buf = [0u8; 1024];
    // SMALL CHUNK FOR FILLING WITH ZERO'S
    let silence = [0u8; 256];

    loop {
        // WAIT TIL DMA BUFFER HAS FREE SPACE
        let free = transfer.available().await.unwrap();
        if free == 0 {
            embassy_time::Timer::after(Duration::from_micros(100)).await;
            continue;
        }

        // CALCULATE HOW MUCH WE CAN READ WITHOUT BORROW CONFLICTS
        let to_read = free.min(pipe_buf.len());
        let read_future = PIPE.read(&mut pipe_buf[..to_read]);
        let timeout = embassy_time::Timer::after(Duration::from_millis(2));

        match select(read_future, timeout).await {
            Either::First(n) if n > 0 => {
                // AUDIO DATA ARRIVED – PUSH TO I2S
                let _ = transfer.push(&pipe_buf[..n]).await;
            }
            _ => {
                // NO DATA – FILL FREE SPACE WITH ZERO'S TO KEEP CLOCKS UP
                let mut remaining = free;
                while remaining > 0 {
                    let chunk = remaining.min(silence.len());
                    let _ = transfer.push(&silence[..chunk]).await;
                    remaining -= chunk;
                }
            }
        }
    }
}



// TCP SERVER TASK TO STREAM AUDIO DATA TO SPEAKER 
// STREAMED AUDIO MUST MATCH DEVICE I2S AUDIO DATA FORMAT EXACTLY 
#[task]
pub async fn stream_speaker(
    stack: &'static embassy_net::Stack<'static>,
    listen_port: u16,
) {
    use embassy_net::tcp::TcpSocket;

    stack.wait_link_up().await;
    stack.wait_config_up().await;
    info!("📡 ☑️ 🔊 Listening on port {}", listen_port);

    loop {
        let mut rx_buffer = [0u8; 4096];
        let mut tx_buffer = [0u8; 2048];
        let mut socket = TcpSocket::new(stack.clone(), &mut rx_buffer, &mut tx_buffer);

        if let Err(e) = socket.accept(listen_port).await {
            error!("accept error: {:?}", e);
            embassy_time::Timer::after(Duration::from_secs(1)).await;
            continue;
        }

        info!("audio client connected");
        socket.set_timeout(Some(Duration::from_secs(30)));

        // READ RAW PCM & FEED THE PIPE
        let mut buf = [0u8; 1024];
        loop {
            match socket.read(&mut buf).await {
                Ok(0) => break, // CLIENT CLOSED
                Ok(n) => {
                    let mut written = 0;
                    while written < n {
                        let w = play(&buf[written..n]);
                        if w == 0 {
                            embassy_time::Timer::after(Duration::from_micros(500)).await;
                        } else {
                            written += w;
                        }
                    }
                }
                Err(e) => {
                    error!("read error: {:?}", e);
                    break;
                }
            }
        }
        info!("client disconnected");
        let _ = socket.close();
    }
}
