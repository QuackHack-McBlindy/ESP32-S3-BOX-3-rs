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
const PLAYBACK_TCP_TX_BUF_SIZE: usize = 1024;
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
            Timer::after(Duration::from_millis(1)).await;
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
            Timer::after(Duration::from_micros(100)).await;
            continue;
        }

        // CALCULATE HOW MUCH WE CAN READ WITHOUT BORROW CONFLICTS
        let to_read = free.min(pipe_buf.len());
        let read_future = PIPE.read(&mut pipe_buf[..to_read]);
        let timeout = Timer::after(Duration::from_millis(2));

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
#[task]
pub async fn stream_speaker(
    stack: &'static embassy_net::Stack<'static>,
    listen_port: u16,
) {
    use embassy_net::tcp::TcpSocket;

    stack.wait_link_up().await;
    stack.wait_config_up().await;

    info!("📡 ☑️ 🔊 listen on port {}", listen_port);

    loop {
        let mut rx_buffer = [0u8; PLAYBACK_TCP_RX_BUF_SIZE];
        let mut tx_buffer = [0u8; PLAYBACK_TCP_TX_BUF_SIZE];
        let mut socket = TcpSocket::new(stack.clone(), &mut rx_buffer, &mut tx_buffer);

        if let Err(e) = socket.accept(listen_port).await {
            error!("accept error: {:?}", e);
            Timer::after(Duration::from_secs(1)).await;
            continue;
        }

        info!("audio client connected from {:?}", socket.remote_endpoint());
        socket.set_timeout(Some(Duration::from_secs(10)));

        'stream: loop {
            // READ 4-BYTE PREFIX (LITTLE-ENDIAN u32)
            let mut len_buf = [0u8; 4];
            let mut read = 0;
            while read < 4 {
                match socket.read(&mut len_buf[read..]).await {
                    Ok(0) => {
                        error!("Connection closed by client");
                        break 'stream;
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        error!("read ERROR: {:?}", e);
                        break 'stream;
                    }
                }
            }
            let sample_count = u32::from_le_bytes(len_buf) as usize;

            if sample_count == 0 || sample_count > 4096 {
                error!("Invalid chunk size: {}", sample_count);
                break 'stream;
            }

            let mut f32_buf = vec![0u8; sample_count * 4];
            let mut read = 0;
            while read < f32_buf.len() {
                match socket.read(&mut f32_buf[read..]).await {
                    Ok(0) => {
                        error!("connection closed mid-chunk");
                        break 'stream;
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        error!("read ERROR: {:?}", e);
                        break 'stream;
                    }
                }
            }

            // CONVERT f32 > i16 > RAW BYTES - PUSH TO RING BUFFER
            let samples_f32: &[f32] = unsafe {
                core::slice::from_raw_parts(
                    f32_buf.as_ptr() as *const f32,
                    sample_count,
                )
            };
            
            assert!(sample_count % 2 == 0, "stereo data must have even number of floats");
            
            let mut pcm_i16 = [0i16; 2048];
            let num_pairs = sample_count / 2;
            for i in 0..num_pairs {
                let left = (samples_f32[2*i].clamp(-1.0, 1.0) * 32767.0) as i16;
                let right = (samples_f32[2*i+1].clamp(-1.0, 1.0) * 32767.0) as i16;
                pcm_i16[2*i] = left;
                pcm_i16[2*i+1] = right;
            }
            let pcm_bytes = unsafe {
                core::slice::from_raw_parts(
                    pcm_i16.as_ptr() as *const u8,
                    num_pairs * 4,
                )
            };

            // PUSH TO SPEAKER
            let mut written = 0;
            while written < pcm_bytes.len() {
                let n = crate::components::speaker::play(&pcm_bytes[written..]);
                if n == 0 {
                    Timer::after(Duration::from_micros(500)).await;
                } else {
                    written += n;
                }
            }
        }

        info!("audio client disconnected");
        let _ = socket.close();
    }
}
