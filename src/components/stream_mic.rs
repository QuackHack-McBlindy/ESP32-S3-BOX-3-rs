// COMPONENTS/STREAM_MIC
// STREAMS MICROPHONE AUDIO DATA TO TCP SERVER
// THAT HANDLES WAKE WORD ++ STT ++ INTENT HANDLE ++ INTENT EXECTUION
// ++ SERVER RETURNS BYTES AS RESPONSE
// ++ & STREAMS TTS DATA BACK TO ESP (COMPONENTS/SPEAKER) 
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
use crate::{init_bool, init_u8, init_u32, init_i8, init_i32, store, load};

const OWW_MODEL_CHUNK_SIZE: usize = 1280;
const TCP_RX_BUF_SIZE: usize = 1024;
const TCP_TX_BUF_SIZE: usize = 4096;
const ROOM: &str = "esp";

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum ASSISTANT_PHASE {
    Disabled = 0,
    Listening = 1,
    Detected  = 2,
    Thinking  = 3,
    Executed  = 4,
    Failed    = 5,
}

init_u8!(ASSISTANT_PHASE, 0);

// MICROPHONE STREAMING TASK
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

    // CONSTRUCT THE MICROPHONE
    let mut mic = crate::components::microphone::Microphone::new(i2s_rx);
    // CONSTRUCT ROOM/DEVICE IDENTIFIER AS BYTES
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

        // STREAM MIC LOOP
        'stream: loop { 
            // SET `LISTENING` PHASE
            store!(ASSISTANT_PHASE, 1);
            // GET NEXT AUDIO CHUNK
            let (chunk, _silent): (Vec<f32>, bool) = match mic.read_chunk().await {
                Ok(pair) => pair,
                Err(e) => {
                    error!("I2S read ERROR: {:?}", e);
                    Timer::after(Duration::from_millis(10)).await;
                    continue;
                }
            };

            // SERIALISE CHUNK 4‑BYTE LENGTH + f32 SAMPLES AS LITTLE-ENDIAN BYTES
            let mut chunk_buffer = vec![0u8; 4 + OWW_MODEL_CHUNK_SIZE * 4];
            chunk_buffer[0..4].copy_from_slice(&(OWW_MODEL_CHUNK_SIZE as u32).to_le_bytes());
            for (i, &sample) in chunk.iter().enumerate() {
                let offset = 4 + i * 4;
                chunk_buffer[offset..offset+4].copy_from_slice(&sample.to_le_bytes());
            }

            // SEND CHUNK TO SERVER
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

            // CHECK SERVER RESPONSE
            let mut byte_buf = [0u8; 1];
            let read_fut = socket.read(&mut byte_buf);
            let timeout_fut = Timer::after(Duration::from_millis(10));
            match select(read_fut, timeout_fut).await {
                embassy_futures::select::Either::First(Ok(1)) => {
                    match byte_buf[0] {
                        0x01 => { // 0x01 == WAKE WORD DETECTED
                            info!("💥 DETECTED Wake Word!");
                            // SET `DETECTED` PHASE
                            store!(ASSISTANT_PHASE, 2);
                            // PLAY DING SOUND
                            crate::components::speaker::play_ding().await;
                            // AND TURN ON DISPLAY
                            crate::components::display::brightness_set("70");
                        } // 0x02 == SERVER STARTED TRANSCRIPTION
                        0x02 => {
                            info!("🧠 THINKING...");
                            // SET `THINKING` PHASE
                            store!(ASSISTANT_PHASE, 3);
                            // FLASH DISPLAY
                            crate::components::display::brightness_set("0");
                            Timer::after(Duration::from_millis(50)).await;
                            crate::components::display::brightness_set("80");
                            Timer::after(Duration::from_millis(50)).await;
                            crate::components::display::brightness_set("0");
                            Timer::after(Duration::from_millis(50)).await;
                            crate::components::display::brightness_set("70");              
                        } // 0x03 == VOICE COMMAND EXECUTED SUCCESSFULLY
                        0x03 => {
                            info!("✅ Executed command!");
                            // SET `EXECUTED` PHASE
                            store!(ASSISTANT_PHASE, 4);
                            // PLAY DONE SOUND
                            crate::components::speaker::play_done().await;
                            // AND TURN OFF DISPLAY
                            crate::components::display::brightness_set("0");
                            // BACK TO `LISTENING` PHASE
                            store!(ASSISTANT_PHASE, 1);
                        } // 0x04 == FAILED VOICE COMMAND EXECUTION
                        0x04 => {
                            info!("💩 FAILED execution!");
                            // SET `FAILED` PHASE
                            store!(ASSISTANT_PHASE, 5);
                            // PLAY DUCK SAY `OH FUCK` SOUND
                            crate::components::speaker::play_fail().await;
                            // AND TURN OFF DISPLAY
                            crate::components::display::brightness_set("0");
                            // BACK TO `LISTENING` PHASE
                            store!(ASSISTANT_PHASE, 1);
                        } // UNEXPECTED RESPONSE
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

        // LOST CONNECTION
        info!("❌ reconnecting...");
        // TRY TO RECONNECT
        let _ = socket.close();
        // EVERY 15 SECONDS
        Timer::after(Duration::from_secs(15)).await;
    }
}
