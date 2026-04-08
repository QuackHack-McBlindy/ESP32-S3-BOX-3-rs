use defmt::{info, error};
use defmt::Debug2Format;
use embassy_executor::task;
use embassy_net::{Stack, tcp::TcpSocket, IpAddress};
use embassy_time::{Duration, Timer};
use embassy_futures::select::select;
use esp_hal::i2s::master::{
    I2sRx,
    asynch::I2sReadDmaTransferAsync,
};
use esp_hal::ledc::channel::{Channel, ChannelIFace};
use esp_hal::ledc::LowSpeed;
use esp_hal::Async;
use core::net::SocketAddr;
use alloc::vec::Vec;
use alloc::vec;
use crate::speaker;



// AUDIO PARAMETERS
const SAMPLE_RATE_HZ: u32 = 16000;
const STEREO_SAMPLES_PER_READ: usize = 256;
const MONO_SAMPLES_PER_READ: usize = STEREO_SAMPLES_PER_READ / 2;
const OWW_MODEL_CHUNK_SIZE: usize = 1280;

// TCP buffers
const TCP_RX_BUF_SIZE: usize = 1024;
const TCP_TX_BUF_SIZE: usize = 4096;

// room identifier (currently only 'esp' supported)
const ROOM: &str = "esp";

//fn //display_brightness(ch: &mut Channel<'static, LowSpeed>, percent: u8) {
//    use esp_hal::ledc::channel::ChannelIFace;
//    let percent = percent.clamp(0, 80);
//    ch.set_duty(percent).unwrap();
//}


#[task]
pub async fn audio_capture_task(
    mut i2s_rx: I2sRx<'static, Async>,
    stack: &'static Stack<'static>,
    remote_addr: SocketAddr,
//    backlight_channel: &'static mut Channel<'static, LowSpeed>,
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

    // I2S buffers
    let mut i2s_buffer = [0u8; STEREO_SAMPLES_PER_READ * 2];
    let mut mono_i16 = [0i16; MONO_SAMPLES_PER_READ];
    let mut mono_f32 = [0f32; MONO_SAMPLES_PER_READ];

    // accumulation buffer for OWW chunks
    let mut accum_buffer = Vec::with_capacity(OWW_MODEL_CHUNK_SIZE);
    let mut chunk_buffer = vec![0u8; 4 + OWW_MODEL_CHUNK_SIZE * 4];

    // handshake
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

        // HANDSHAKE: send room length and name
        let mut handshake_ok = true;

        let len_bytes = room_len.to_le_bytes();
        let mut written = 0;
        while written < len_bytes.len() {
            match socket.write(&len_bytes[written..]).await {
                Ok(n) => written += n,
                Err(e) => {
                    error!("Handshake length failed: {:?}", e);
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

        // reset accumulation buffer
        accum_buffer.clear();

        // STREAMING LOOP: send wake chunks
        'stream: loop {
            let mut silent = false;
            if let Err(e) = i2s_rx.read_dma_async(&mut i2s_buffer).await {
                error!("I2S read error: {:?}", e);
                Timer::after(Duration::from_millis(10)).await;
                continue;
            }

            // stereo i16 > mono i16 > mono f32
            let stereo = unsafe {
                core::slice::from_raw_parts(
                    i2s_buffer.as_ptr() as *const i16,
                    STEREO_SAMPLES_PER_READ,
                )
            };
            for (i, chunk) in stereo.chunks(2).enumerate() {
                mono_i16[i] = ((chunk[0] as i32 + chunk[1] as i32) / 2) as i16;
            }
            for (i, &s) in mono_i16.iter().enumerate() {
                mono_f32[i] = s as f32 / 32768.0;
            }

            // add to accumulation buffer
            accum_buffer.extend_from_slice(&mono_f32[..MONO_SAMPLES_PER_READ]);

            // send when enough samples for one wake word chunk
            if accum_buffer.len() >= OWW_MODEL_CHUNK_SIZE {
                let chunk_len = OWW_MODEL_CHUNK_SIZE;
                chunk_buffer[0..4].copy_from_slice(&(chunk_len as u32).to_le_bytes());
                for (i, &sample) in accum_buffer.iter().take(chunk_len).enumerate() {
                    let offset = 4 + i * 4;
                    chunk_buffer[offset..offset+4].copy_from_slice(&sample.to_le_bytes());
                }

                let data = &chunk_buffer[..4 + chunk_len*4];

                // microphone check - one two - mic check one two
                let audio_data = &data[4..];
                let all_zero = audio_data.iter().all(|&b| b == 0);
                // one two zero ?
                if all_zero { // all zero is not good
                    if !silent {
                        info!("🎙️⚠️ Microphone needs attention!");
                        silent = true;
                    } // finally non all zero
                } else { 
                    if silent { 
                        info!("🎙️✅ Mic OK!");
                        silent = false; 
                    }
                }

                
                let mut written = 0;
                while written < data.len() {
                    match socket.write(&data[written..]).await {
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

                // TODO; discard leftovers instead?
                if accum_buffer.len() > OWW_MODEL_CHUNK_SIZE {
                    let remaining = accum_buffer.split_off(OWW_MODEL_CHUNK_SIZE);
                    accum_buffer = remaining;
                } else {
                    accum_buffer.clear();
                }

                // DETECTED check
                let mut byte_buf = [0u8; 1];
                let read_fut = socket.read(&mut byte_buf);
                let timeout_fut = Timer::after(Duration::from_millis(10));
                match select(read_fut, timeout_fut).await {
                    embassy_futures::select::Either::First(Ok(1)) => {
                        let byte = byte_buf[0];
                        match byte {
                            0x01 => {
                                info!("💥 DETECTED Wake Word!");
                                //display_brightness(backlight_channel, 70);
                                speaker::play_ding().await;
                            }
                            0x03 => {
                                info!("✅ Executed command!");
                                //display_brightness(backlight_channel, 0);
                                speaker::play_done().await;
                            }
                            0x04 => {
                                info!("💩 FAILED execution!");
                                //display_brightness(backlight_channel, 0);
                                speaker::play_fail().await;
                            }
                            _ => info!("Unexpected byte from server: 0x{:02x}", byte),
                        }
                    }
                    embassy_futures::select::Either::First(Ok(_)) => {
                        // ignore
                    }
                    embassy_futures::select::Either::First(Err(e)) => {
                        error!("socket read error: {:?}", e);
                        break 'stream;
                    }
                    embassy_futures::select::Either::Second(_) => {
                        // TIMEOUT
                    }
                }
            }
        }

        info!("❌ reconnecting...");
        let _ = socket.close();
        Timer::after(Duration::from_secs(15)).await;
    }
}
