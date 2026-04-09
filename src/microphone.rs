use defmt::{info, error};
use embassy_executor::task;
use embassy_net::{Stack, tcp::TcpSocket, IpAddress};
use embassy_time::{Duration, Timer};
use embassy_futures::select::select;
use esp_hal::i2s::master::I2sRx;
use esp_hal::Async;
use core::net::SocketAddr;
use alloc::vec::Vec;
use alloc::vec;
use crate::speaker;
use crate::media;
use crate::mic::Microphone;

const OWW_MODEL_CHUNK_SIZE: usize = 1280;
const TCP_RX_BUF_SIZE: usize = 1024;
const TCP_TX_BUF_SIZE: usize = 4096;
const ROOM: &str = "esp";

#[task]
pub async fn audio_capture_task(
    i2s_rx: I2sRx<'static, Async>,
    stack: &'static Stack<'static>,
    remote_addr: SocketAddr,
) {
    let remote_endpoint = match remote_addr {
        SocketAddr::V4(v4) => (IpAddress::Ipv4(v4.ip().octets().into()), v4.port()),
        SocketAddr::V6(_) => {
            error!("IPv6 not supported");
            return;
        }
    };

    stack.wait_link_up().await;
    stack.wait_config_up().await;

    let mut mic = Microphone::new(i2s_rx);
    let room_bytes = ROOM.as_bytes();
    let room_len = room_bytes.len() as u32;

    loop {
        let mut rx_buffer = [0u8; TCP_RX_BUF_SIZE];
        let mut tx_buffer = [0u8; TCP_TX_BUF_SIZE];
        let mut socket = TcpSocket::new(stack.clone(), &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        if let Err(e) = socket.connect(remote_endpoint).await {
            error!("❌ connect error: {:?}, retrying in 15s", e);
            Timer::after(Duration::from_secs(15)).await;
            continue;
        }
        info!("📡 ☑️ 🎙️ to {}", remote_addr);

        // SHAKE HANDS!
        let mut handshake_ok = true;
        let len_bytes = room_len.to_le_bytes();
        let mut written = 0;
        while written < len_bytes.len() {
            match socket.write(&len_bytes[written..]).await {
                Ok(n) => written += n,
                Err(e) => {
                    error!("handshake length fail: {:?}", e);
                    handshake_ok = false;
                    break;
                }
            }
        }
        if handshake_ok && room_len > 0 {
            let mut written = 0;
            while written < room_bytes.len() {
                match socket.write(&room_bytes[written..]).await {
                    Ok(n) => written += n,
                    Err(e) => {
                        error!("failed to send room name: {:?}", e);
                        handshake_ok = false;
                        break;
                    }
                }
            }
        }
        if let Err(e) = socket.flush().await {
            error!("failed to flush handshake: {:?}", e);
            handshake_ok = false;
        }
        if !handshake_ok {
            let _ = socket.close();
            Timer::after(Duration::from_secs(15)).await;
            continue;
        }

        // STREAM MIC
        'stream: loop {
            // get next audio chunk – explicit type annotation fixes inference
            let (chunk, _silent): (Vec<f32>, bool) = match mic.read_chunk().await {
                Ok(pair) => pair,
                Err(e) => {
                    error!("I2S read error: {:?}", e);
                    Timer::after(Duration::from_millis(10)).await;
                    continue;
                }
            };

            // serialise chunk: 4‑byte length + f32 samples as little‑endian bytes
            let mut chunk_buffer = vec![0u8; 4 + OWW_MODEL_CHUNK_SIZE * 4];
            chunk_buffer[0..4].copy_from_slice(&(OWW_MODEL_CHUNK_SIZE as u32).to_le_bytes());
            for (i, &sample) in chunk.iter().enumerate() {
                let offset = 4 + i * 4;
                chunk_buffer[offset..offset+4].copy_from_slice(&sample.to_le_bytes());
            }

            // send
            let mut written = 0;
            while written < chunk_buffer.len() {
                match socket.write(&chunk_buffer[written..]).await {
                    Ok(n) => written += n,
                    Err(e) => {
                        error!("failed to send audio chunk: {:?}", e);
                        break 'stream;
                    }
                }
            }
            if let Err(e) = socket.flush().await {
                error!("Failed to flush! {:?}", e);
                break 'stream;
            }

            // SERVER RESPONSE ?
            let mut byte_buf = [0u8; 1];
            let read_fut = socket.read(&mut byte_buf);
            let timeout_fut = Timer::after(Duration::from_millis(10));
            match select(read_fut, timeout_fut).await {
                embassy_futures::select::Either::First(Ok(1)) => {
                    match byte_buf[0] {
                        0x01 => {
                            media::on_wake_word_detected();
                        }
                        0x03 => {
                            media::on_command_executed();
                        }
                        0x04 => {
                            media::on_command_failed();
                        }
                        _ => info!("Unexpected byte: 0x{:02x}", byte_buf[0]),
                    }
                }
                embassy_futures::select::Either::First(Ok(_)) => {}
                embassy_futures::select::Either::First(Err(e)) => {
                    error!("socket read error: {:?}", e);
                    break 'stream;
                }
                embassy_futures::select::Either::Second(_) => {}
            }
        }

        info!("❌ reconnecting...");
        let _ = socket.close();
        Timer::after(Duration::from_secs(15)).await;
    }
}
