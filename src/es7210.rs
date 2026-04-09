#![no_std]
#![allow(unused)]

use embedded_hal::i2c::I2c;

// Register addresses
const RESET_REG00: u8 = 0x00;
const CLOCK_OFF_REG01: u8 = 0x01;
const MAINCLK_REG02: u8 = 0x02;
const MASTER_CLK_REG03: u8 = 0x03;
const LRCK_DIVH_REG04: u8 = 0x04;
const LRCK_DIVL_REG05: u8 = 0x05;
const POWER_DOWN_REG06: u8 = 0x06;
const OSR_REG07: u8 = 0x07;
const MODE_CONFIG_REG08: u8 = 0x08;
const TIME_CONTROL0_REG09: u8 = 0x09;
const TIME_CONTROL1_REG0A: u8 = 0x0A;
const SDP_INTERFACE1_REG11: u8 = 0x11;
const SDP_INTERFACE2_REG12: u8 = 0x12;
const ADC_AUTOMUTE_REG13: u8 = 0x13;
const ADC34_MUTERANGE_REG14: u8 = 0x14;
const ALC_SEL_REG16: u8 = 0x16;
const ADC1_DIRECT_DB_REG1B: u8 = 0x1B;
const ADC2_DIRECT_DB_REG1C: u8 = 0x1C;
const ADC3_DIRECT_DB_REG1D: u8 = 0x1D;
const ADC4_DIRECT_DB_REG1E: u8 = 0x1E;
const ADC34_HPF2_REG20: u8 = 0x20;
const ADC34_HPF1_REG21: u8 = 0x21;
const ADC12_HPF2_REG22: u8 = 0x22;
const ADC12_HPF1_REG23: u8 = 0x23;
const ANALOG_REG40: u8 = 0x40;
const MIC12_BIAS_REG41: u8 = 0x41;
const MIC34_BIAS_REG42: u8 = 0x42;
const MIC1_GAIN_REG43: u8 = 0x43;
const MIC2_GAIN_REG44: u8 = 0x44;
const MIC3_GAIN_REG45: u8 = 0x45;
const MIC4_GAIN_REG46: u8 = 0x46;
const MIC1_POWER_REG47: u8 = 0x47;
const MIC2_POWER_REG48: u8 = 0x48;
const MIC3_POWER_REG49: u8 = 0x49;
const MIC4_POWER_REG4A: u8 = 0x4A;
const MIC12_POWER_REG4B: u8 = 0x4B;
const MIC34_POWER_REG4C: u8 = 0x4C;


// Types and enums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum I2sFormat {
    I2S = 0x00,
    LeftJustified = 0x01,
    DspA = 0x03,
    DspB = 0x13,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum I2sBits {
    Bits16 = 16,
    Bits18 = 18,
    Bits20 = 20,
    Bits24 = 24,
    Bits32 = 32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MicGain {
    Gain0dB = 0,
    Gain3dB = 1,
    Gain6dB = 2,
    Gain9dB = 3,
    Gain12dB = 4,
    Gain15dB = 5,
    Gain18dB = 6,
    Gain21dB = 7,
    Gain24dB = 8,
    Gain27dB = 9,
    Gain30dB = 10,
    Gain33dB = 11,
    Gain34_5dB = 12,
    Gain36dB = 13,
    Gain37_5dB = 14,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MicBias {
    V2_18 = 0x00,
    V2_26 = 0x10,
    V2_36 = 0x20,
    V2_45 = 0x30,
    V2_55 = 0x40,
    V2_66 = 0x50,
    V2_78 = 0x60,
    V2_87 = 0x70,
}

#[derive(Debug, Clone, Copy)]
pub struct CodecConfig {
    pub sample_rate_hz: u32,
    pub mclk_ratio: u32,
    pub i2s_format: I2sFormat,
    pub bit_width: I2sBits,
    pub mic_bias: MicBias,
    pub mic_gain: MicGain,
    pub tdm_enable: bool,
}

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    InvalidConfig,
    NotSupported,
}

/// Stateless driver
pub struct Es7210 {
    addr: u8,
}

impl Es7210 {
    pub fn new(addr: u8) -> Self {
        Self { addr }
    }

    fn write_reg<I2C, E>(&self, i2c: &mut I2C, reg: u8, val: u8) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        let buf = [reg, val];
        i2c.write(self.addr, &buf).map_err(Error::I2c)
    }

    pub fn config_codec<I2C, E>(
        &self,
        i2c: &mut I2C,
        cfg: &CodecConfig,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        // Software reset
        self.write_reg(i2c, RESET_REG00, 0xFF)?;
        self.write_reg(i2c, RESET_REG00, 0x32)?;

        // Set power-up timing
        self.write_reg(i2c, TIME_CONTROL0_REG09, 0x30)?;
        self.write_reg(i2c, TIME_CONTROL1_REG0A, 0x30)?;

        // HPF configuration
        self.write_reg(i2c, ADC12_HPF1_REG23, 0x2A)?;
        self.write_reg(i2c, ADC12_HPF2_REG22, 0x0A)?;
        self.write_reg(i2c, ADC34_HPF1_REG21, 0x2A)?;
        self.write_reg(i2c, ADC34_HPF2_REG20, 0x0A)?;

        // I2S format and bit width
        self.set_i2s_format(i2c, cfg.i2s_format, cfg.bit_width, cfg.tdm_enable)?;

        // Analog power and VMID
        self.write_reg(i2c, ANALOG_REG40, 0xC3)?;

        // MIC bias
        self.set_mic_bias(i2c, cfg.mic_bias)?;

        // MIC gain
        self.set_mic_gain(i2c, cfg.mic_gain)?;

        // Power up MICs
        self.write_reg(i2c, MIC1_POWER_REG47, 0x08)?;
        self.write_reg(i2c, MIC2_POWER_REG48, 0x08)?;
        self.write_reg(i2c, MIC3_POWER_REG49, 0x08)?;
        self.write_reg(i2c, MIC4_POWER_REG4A, 0x08)?;

        // Sample rate and clock
        self.set_sample_rate(i2c, cfg.sample_rate_hz, cfg.mclk_ratio)?;

        // Power down DLL
        self.write_reg(i2c, POWER_DOWN_REG06, 0x04)?;

        // Power up ADCs and PGAs
        self.write_reg(i2c, MIC12_POWER_REG4B, 0x0F)?;
        self.write_reg(i2c, MIC34_POWER_REG4C, 0x0F)?;

        // Enable device
        self.write_reg(i2c, RESET_REG00, 0x71)?;
        self.write_reg(i2c, RESET_REG00, 0x41)?;

        Ok(())
    }

    /// Enable the codec (power up, start ADCs)
    pub fn enable<I2C, E>(&self, i2c: &mut I2C) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        self.write_reg(i2c, RESET_REG00, 0x71)?;
        self.write_reg(i2c, RESET_REG00, 0x41)?;
        Ok(())
    }

    /// Disable the codec (power down, stop ADCs)
    pub fn disable<I2C, E>(&self, i2c: &mut I2C) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        // Put the codec into reset / power‑down state
        self.write_reg(i2c, RESET_REG00, 0x00)?;
        Ok(())
    }


    pub fn gain_set<I2C, E>(
        &self,
        i2c: &mut I2C,
        volume_db: i8,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        if !(-95..=32).contains(&volume_db) {
            return Err(Error::InvalidConfig);
        }
        let reg_val = (191 + (volume_db as i32) * 2) as u8;
        self.write_reg(i2c, ADC1_DIRECT_DB_REG1B, reg_val)?;
        self.write_reg(i2c, ADC2_DIRECT_DB_REG1C, reg_val)?;
        self.write_reg(i2c, ADC3_DIRECT_DB_REG1D, reg_val)?;
        self.write_reg(i2c, ADC4_DIRECT_DB_REG1E, reg_val)?;
        Ok(())
    }


    // Private helpers
    fn set_i2s_format<I2C, E>(
        &self,
        i2c: &mut I2C,
        fmt: I2sFormat,
        bits: I2sBits,
        tdm: bool,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        let bits_val = match bits {
            I2sBits::Bits16 => 0x60,
            I2sBits::Bits18 => 0x40,
            I2sBits::Bits20 => 0x20,
            I2sBits::Bits24 => 0x00,
            I2sBits::Bits32 => 0x80,
        };
        self.write_reg(i2c, SDP_INTERFACE1_REG11, fmt as u8 | bits_val)?;

        let reg12 = if tdm {
            match fmt {
                I2sFormat::I2S | I2sFormat::LeftJustified => 0x02,
                I2sFormat::DspA | I2sFormat::DspB => 0x01,
            }
        } else {
            0x00
        };
        self.write_reg(i2c, SDP_INTERFACE2_REG12, reg12)?;

        defmt::info!(
            "I2S format: {}, bit width: {}, TDM: {}",
            match fmt {
                I2sFormat::I2S => "I2S",
                I2sFormat::LeftJustified => "LJ",
                I2sFormat::DspA => "DSP-A",
                I2sFormat::DspB => "DSP-B",
            },
            bits as u8,
            tdm
        );
        Ok(())
    }

    fn set_mic_gain<I2C, E>(
        &self,
        i2c: &mut I2C,
        gain: MicGain,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        let val = (gain as u8) | 0x10;
        self.write_reg(i2c, MIC1_GAIN_REG43, val)?;
        self.write_reg(i2c, MIC2_GAIN_REG44, val)?;
        self.write_reg(i2c, MIC3_GAIN_REG45, val)?;
        self.write_reg(i2c, MIC4_GAIN_REG46, val)?;
        Ok(())
    }

    fn set_mic_bias<I2C, E>(
        &self,
        i2c: &mut I2C,
        bias: MicBias,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        self.write_reg(i2c, MIC12_BIAS_REG41, bias as u8)?;
        self.write_reg(i2c, MIC34_BIAS_REG42, bias as u8)?;
        Ok(())
    }

    fn set_sample_rate<I2C, E>(
        &self,
        i2c: &mut I2C,
        sample_rate: u32,
        mclk_ratio: u32,
    ) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
    {
        let mclk = sample_rate * mclk_ratio;
        let coeff = COEFF_TABLE
            .iter()
            .find(|c| c.mclk == mclk && c.lrck == sample_rate)
            .ok_or(Error::NotSupported)?;

        self.write_reg(i2c, OSR_REG07, coeff.osr)?;
        self.write_reg(i2c, MAINCLK_REG02, coeff.adc_div | (coeff.doubler << 6) | (coeff.dll << 7))?;
        self.write_reg(i2c, LRCK_DIVH_REG04, coeff.lrck_h as u8)?;
        self.write_reg(i2c, LRCK_DIVL_REG05, coeff.lrck_l as u8)?;

        defmt::info!("Sample rate: {} Hz, MCLK: {} Hz", sample_rate, mclk);
        Ok(())
    }
}


// Clock coefficient table
#[derive(Clone, Copy)]
struct CoeffDiv {
    mclk: u32,
    lrck: u32,
    _ss_ds: u8,
    adc_div: u8,
    dll: u8,
    doubler: u8,
    osr: u8,
    _mclk_src: u8,
    lrck_h: u32,
    lrck_l: u32,
}


const COEFF_TABLE: &[CoeffDiv] = &[
    // 8k
    CoeffDiv {
        mclk: 12288000,
        lrck: 8000,
        _ss_ds: 0x00,
        adc_div: 0x03,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x06,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 16384000,
        lrck: 8000,
        _ss_ds: 0x00,
        adc_div: 0x04,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x08,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 8000,
        _ss_ds: 0x00,
        adc_div: 0x1e,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x28,
        _mclk_src: 0x00,
        lrck_h: 0x09,
        lrck_l: 0x60,
    },
    CoeffDiv {
        mclk: 4096000,
        lrck: 8000,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x02,
        lrck_l: 0x00,
    },
    // 11.025k
    CoeffDiv {
        mclk: 11289600,
        lrck: 11025,
        _ss_ds: 0x00,
        adc_div: 0x02,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x00,
    },
    // 12k
    CoeffDiv {
        mclk: 12288000,
        lrck: 12000,
        _ss_ds: 0x00,
        adc_div: 0x02,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x04,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 12000,
        _ss_ds: 0x00,
        adc_div: 0x14,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x28,
        _mclk_src: 0x00,
        lrck_h: 0x06,
        lrck_l: 0x40,
    },
    // 16k
    CoeffDiv {
        mclk: 4096000,
        lrck: 16000,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 16000,
        _ss_ds: 0x00,
        adc_div: 0x0a,
        dll: 0x00,
        doubler: 0x00,
        osr: 0x1e,
        _mclk_src: 0x00,
        lrck_h: 0x04,
        lrck_l: 0x80,
    },
    CoeffDiv {
        mclk: 16384000,
        lrck: 16000,
        _ss_ds: 0x00,
        adc_div: 0x02,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x04,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 12288000,
        lrck: 16000,
        _ss_ds: 0x00,
        adc_div: 0x03,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x03,
        lrck_l: 0x00,
    },
    // 22.05k
    CoeffDiv {
        mclk: 11289600,
        lrck: 22050,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x02,
        lrck_l: 0x00,
    },
    // 24k
    CoeffDiv {
        mclk: 12288000,
        lrck: 24000,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x02,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 24000,
        _ss_ds: 0x00,
        adc_div: 0x0a,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x28,
        _mclk_src: 0x00,
        lrck_h: 0x03,
        lrck_l: 0x20,
    },
    // 32k
    CoeffDiv {
        mclk: 12288000,
        lrck: 32000,
        _ss_ds: 0x00,
        adc_div: 0x03,
        dll: 0x00,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x80,
    },
    CoeffDiv {
        mclk: 16384000,
        lrck: 32000,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x02,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 32000,
        _ss_ds: 0x00,
        adc_div: 0x05,
        dll: 0x00,
        doubler: 0x00,
        osr: 0x1e,
        _mclk_src: 0x00,
        lrck_h: 0x02,
        lrck_l: 0x58,
    },
    // 44.1k
    CoeffDiv {
        mclk: 11289600,
        lrck: 44100,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x00,
    },
    // 48k
    CoeffDiv {
        mclk: 12288000,
        lrck: 48000,
        _ss_ds: 0x00,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 48000,
        _ss_ds: 0x00,
        adc_div: 0x05,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x28,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x90,
    },
    // 64k
    CoeffDiv {
        mclk: 16384000,
        lrck: 64000,
        _ss_ds: 0x01,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x00,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x00,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 64000,
        _ss_ds: 0x00,
        adc_div: 0x05,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x1e,
        _mclk_src: 0x00,
        lrck_h: 0x01,
        lrck_l: 0x2c,
    },
    // 88.2k
    CoeffDiv {
        mclk: 11289600,
        lrck: 88200,
        _ss_ds: 0x01,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x00,
        lrck_l: 0x80,
    },
    // 96k
    CoeffDiv {
        mclk: 12288000,
        lrck: 96000,
        _ss_ds: 0x01,
        adc_div: 0x01,
        dll: 0x01,
        doubler: 0x01,
        osr: 0x20,
        _mclk_src: 0x00,
        lrck_h: 0x00,
        lrck_l: 0x80,
    },
    CoeffDiv {
        mclk: 19200000,
        lrck: 96000,
        _ss_ds: 0x01,
        adc_div: 0x05,
        dll: 0x00,
        doubler: 0x01,
        osr: 0x28,
        _mclk_src: 0x00,
        lrck_h: 0x00,
        lrck_l: 0xc8,
    },
];
