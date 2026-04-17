use defmt::info;
use esp_hal::i2s::master::{I2sRx, asynch::I2sReadDmaTransferAsync};
use esp_hal::Async;
use alloc::vec::Vec;

const STEREO_SAMPLES_PER_READ: usize = 256;
const MONO_SAMPLES_PER_READ: usize = STEREO_SAMPLES_PER_READ / 2;
const OWW_MODEL_CHUNK_SIZE: usize = 1280;

pub struct Microphone {
    i2s_rx: I2sRx<'static, Async>,
    stereo_buffer: [u8; STEREO_SAMPLES_PER_READ * 2],
    mono_i16: [i16; MONO_SAMPLES_PER_READ],
    mono_f32: [f32; MONO_SAMPLES_PER_READ],
    accum_buffer: Vec<f32>,
    silent: bool,
}

impl Microphone {
    pub fn new(i2s_rx: I2sRx<'static, Async>) -> Self {
        Self {
            i2s_rx,
            stereo_buffer: [0u8; STEREO_SAMPLES_PER_READ * 2],
            mono_i16: [0i16; MONO_SAMPLES_PER_READ],
            mono_f32: [0f32; MONO_SAMPLES_PER_READ],
            accum_buffer: Vec::with_capacity(OWW_MODEL_CHUNK_SIZE),
            silent: false,
        }
    }


    pub async fn read_chunk(&mut self) -> Result<(Vec<f32>, bool), ()> {
        while self.accum_buffer.len() < OWW_MODEL_CHUNK_SIZE {

            if let Err(_) = self.i2s_rx.read_dma_async(&mut self.stereo_buffer).await {
            //if let Err(_) = self.i2s_rx.read_dma_circular_async(&mut self.stereo_buffer).await {                        
                return Err(());
            }

            let stereo = unsafe {
                core::slice::from_raw_parts(
                    self.stereo_buffer.as_ptr() as *const i16,
                    STEREO_SAMPLES_PER_READ,
                )
            };
            for (i, chunk) in stereo.chunks(2).enumerate() {
                self.mono_i16[i] = ((chunk[0] as i32 + chunk[1] as i32) / 2) as i16;
            }
            for (i, &s) in self.mono_i16.iter().enumerate() {
                self.mono_f32[i] = s as f32 / 32768.0;
            }
            self.accum_buffer.extend_from_slice(&self.mono_f32[..MONO_SAMPLES_PER_READ]);
        }

        let chunk: Vec<f32> = self.accum_buffer.drain(..OWW_MODEL_CHUNK_SIZE).collect();

        let all_zero = chunk.iter().all(|&s| s == 0.0);
        if all_zero {
            if !self.silent {
                info!("🎙️⚠️ Mic zero zero zero!");
                self.silent = true;
            }
        } else if self.silent {
            info!("🎙️✅ Mic OK!");
            self.silent = false;
        }
        Ok((chunk, all_zero))
    }
}
