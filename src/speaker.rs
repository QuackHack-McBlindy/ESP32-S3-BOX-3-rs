use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::{Duration, Timer};
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use esp_hal::i2s::master::I2sTx;
use esp_hal::Async;
use defmt::{info, error};
use alloc::vec;


pub const RING_BUFFER_SIZE: usize = 16384;
const DMA_BUFFER_SIZE: usize = 2048;

static PIPE: Pipe<CriticalSectionRawMutex, RING_BUFFER_SIZE> = Pipe::new();

const DING_SOUND: &[u8] = include_bytes!("./../assets/sound/ding_esp.raw");
const DONE_SOUND: &[u8] = include_bytes!("./../assets/sound/done_esp.wav");
const FAIL_SOUND: &[u8] = include_bytes!("./../assets/sound/fail_esp.wav");

pub fn play(data: &[u8]) -> usize {
    PIPE.try_write(data).unwrap_or(0)
}

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
        } else {
            Timer::after(Duration::from_millis(10)).await;
        }
    }
}

const PLAYBACK_TCP_RX_BUF_SIZE: usize = 4096;
const PLAYBACK_TCP_TX_BUF_SIZE: usize = 1024;

#[task]
pub async fn audio_playback_task(
    stack: &'static embassy_net::Stack<'static>,
    server_addr: core::net::SocketAddr,
) {
    use embassy_net::{IpAddress, tcp::TcpSocket};
    use embassy_futures::select::select;

    let remote_endpoint = match server_addr {
        core::net::SocketAddr::V4(v4) => (IpAddress::Ipv4(v4.ip().octets().into()), v4.port()),
        core::net::SocketAddr::V6(_) => {
            error!("IPv6 not supported for playback");
            return;
        }
    };

    stack.wait_link_up().await;
    stack.wait_config_up().await;

    loop {
        let mut rx_buffer = [0u8; PLAYBACK_TCP_RX_BUF_SIZE];
        let mut tx_buffer = [0u8; PLAYBACK_TCP_TX_BUF_SIZE];
        let mut socket = TcpSocket::new(stack.clone(), &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        info!("🔊 Connecting to audio source at {} ...", server_addr);
        if let Err(e) = socket.connect(remote_endpoint).await {
            error!("Playback connect error: {:?}, retry in 15s", e);
            Timer::after(Duration::from_secs(15)).await;
            continue;
        }
        info!("Playback connected – ready to receive audio");

        'stream: loop {
            // read 4-byte length prefix
            let mut len_buf = [0u8; 4];
            let mut read = 0;
            while read < 4 {
                match socket.read(&mut len_buf[read..]).await {
                    Ok(0) => {
                        error!("Connection closed by server");
                        break 'stream;
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        error!("Playback read error: {:?}", e);
                        break 'stream;
                    }
                }
            }
            let sample_count = u32::from_le_bytes(len_buf) as usize;
            if sample_count == 0 || sample_count > 4096 {
                error!("Invalid chunk size: {}", sample_count);
                break 'stream;
            }

            // read f32 samples
            let mut f32_buf = vec![0u8; sample_count * 4];
            let mut read = 0;
            while read < f32_buf.len() {
                match socket.read(&mut f32_buf[read..]).await {
                    Ok(0) => {
                        error!("Connection closed mid‑chunk");
                        break 'stream;
                    }
                    Ok(n) => read += n,
                    Err(e) => {
                        error!("Playback read error: {:?}", e);
                        break 'stream;
                    }
                }
            }

            // Convert f32 > i16 > raw bytes
            let samples_f32: &[f32] = unsafe {
                core::slice::from_raw_parts(
                    f32_buf.as_ptr() as *const f32,
                    sample_count,
                )
            };
            let mut pcm_i16 = [0i16; 1024];
            for (i, &f) in samples_f32.iter().enumerate() {
                let clamped = f.clamp(-1.0, 1.0);
                pcm_i16[i] = (clamped * 32767.0) as i16;
            }
            let pcm_bytes = unsafe {
                core::slice::from_raw_parts(
                    pcm_i16.as_ptr() as *const u8,
                    sample_count * 2,
                )
            };

            let mut written = 0;
            while written < pcm_bytes.len() {
                let n = crate::speaker::play(&pcm_bytes[written..]);
                if n == 0 {
                    Timer::after(Duration::from_micros(500)).await;
                } else {
                    written += n;
                }
            }
        }

        info!("Playback disconnected – reconnecting in 5s");
        let _ = socket.close();
        Timer::after(Duration::from_secs(5)).await;
    }
}*
