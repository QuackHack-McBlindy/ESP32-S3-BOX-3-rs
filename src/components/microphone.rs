// COMPONENTS/MICROPHONE
// CONSTRUCT MICROPHONE
// AND PROVIDE A `read_chunk` FUNCTION
use defmt::info;
use defmt::Debug2Format;
use esp_hal::i2s::master::{I2sRx, asynch::I2sReadDmaTransferAsync};
use esp_hal::Async;
use alloc::vec::Vec;

const STEREO_SAMPLES_PER_READ: usize = 256;
const MONO_SAMPLES_PER_READ: usize = STEREO_SAMPLES_PER_READ / 2;
// MUST MATCH WAKE WORD CHUNK SIZE
const OWW_MODEL_CHUNK_SIZE: usize = 1280;
// DEBUG WILL FLOOD LOGS
const DEBUG_MIC: bool = false;


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

    // READ CHUNK FROM MICROPHONE
    pub async fn read_chunk(&mut self) -> Result<(Vec<f32>, bool), ()> {
        while self.accum_buffer.len() < OWW_MODEL_CHUNK_SIZE {
            match self.i2s_rx.read_dma_async(&mut self.stereo_buffer).await {
                Ok(()) => {}
                Err(e) => {
                    defmt::error!("I2S read_dma_async failed: {}", Debug2Format(&e));
                    return Err(());
                }
            }

            // DETAILED DEBUG (PRINTING FIRST 8 RAW BYTES) 
            if DEBUG_MIC { // THIS WILL FLOOD LOGS
                let stereo = unsafe {
                    core::slice::from_raw_parts(
                        self.stereo_buffer.as_ptr() as *const i16,
                        STEREO_SAMPLES_PER_READ,
                    )
                };
                info!("[MIC i16]: {:?}", &stereo[..8.min(stereo.len())]);
            }

            // REINTERPRET BYTE BUFFER AS SLICE OF I16 SAMPLES (STEREO L, R, L, R ...)
            let stereo = unsafe {
                core::slice::from_raw_parts(
                    self.stereo_buffer.as_ptr() as *const i16,
                    STEREO_SAMPLES_PER_READ,
                )
            };
            
            // CONVERT STEREO TO MONO BY AVERAGING EACH LEFT/RIGHT PAIR MONO == (L + R) / 2
            // THIS IS DONE IN I32 TO AVOID OVERFLOW DURING ADDITION THEN CAST BACK TO I16
            for (i, chunk) in stereo.chunks(2).enumerate() {
                self.mono_i16[i] = ((chunk[0] as i32 + chunk[1] as i32) / 2) as i16;
            }
            
            // NORMALIZE I16 SAMPLES TO F32 IN RANGE [-1.0, 1.0] BY DIVIDING BY 32768.0
            // (THE MAXIMUM POSITIVE VALUE OF I16 + 1 TO AVOID ASYMMETRIC SCALING)
            for (i, &s) in self.mono_i16.iter().enumerate() {
                self.mono_f32[i] = s as f32 / 32768.0;
            }
            self.accum_buffer.extend_from_slice(&self.mono_f32[..MONO_SAMPLES_PER_READ]);
        }

        let chunk: Vec<f32> = self.accum_buffer.drain(..OWW_MODEL_CHUNK_SIZE).collect();

        // SILENCE DETECTION
        let all_zero = chunk.iter().all(|&s| s == 0.0);
        if all_zero { // IF DATA IS ALL ZERO
            if !self.silent { // SOMETHING IS WRONG
                info!("🎙️⚠️ Mic zero zero zero!");
                self.silent = true;
            } // READING NON-ZERO VALUES AGAIN
        } else if self.silent {
            info!("🎙️✅ Mic OK!");
            self.silent = false;
        }
        Ok((chunk, all_zero))
    }
}
