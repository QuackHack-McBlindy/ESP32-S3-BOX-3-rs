#![no_std]

use core::ptr::{read_volatile, write_volatile, addr_of_mut};

// ----------------------------------------------------------------------------
//  Error type (must be defined before I2sChannel)
// ----------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EspError {
    Ok = 0,
    ErrInvalidArg = 0x102,
    ErrInvalidState = 0x103,
    ErrTimeout = 0x107,
}
pub type TickType_t = u32;
pub const NON_BLOCK: TickType_t = 0;

// ----------------------------------------------------------------------------
//  Constants
// ----------------------------------------------------------------------------
const I2S0_BASE: *mut u32 = 0x6000_4000 as *mut u32;
const DMA_BASE: *mut u32 = 0x6000_8000 as *mut u32;

const I2S_CONF0_REG: usize = 0x00;
const I2S_RX_CONF_REG: usize = 0x10;
const I2S_TX_CONF_REG: usize = 0x14;
const I2S_RX_CLKM_CONF_REG: usize = 0x18;
const I2S_TX_CLKM_CONF_REG: usize = 0x1C;
const I2S_CLKM_CONF_REG: usize = 0x20;
const I2S_FIFO_CONF_REG: usize = 0x24;
const I2S_RX_EOF_DES_ADDR_REG: usize = 0x30;
const I2S_TX_EOF_DES_ADDR_REG: usize = 0x34;
const I2S_RX_CONF1_REG: usize = 0x38;
const I2S_TX_CONF1_REG: usize = 0x3C;

// DMA channels 2 (RX) and 3 (TX)
const DMA_IN_CONF0_CH2: usize = 0x400;
const DMA_IN_LINK_CH2: usize = 0x404;
const DMA_IN_INT_RAW_CH2: usize = 0x40C;
const DMA_IN_INT_CLR_CH2: usize = 0x410;
const DMA_IN_INT_ENA_CH2: usize = 0x414;
const DMA_OUT_CONF0_CH3: usize = 0x500;
const DMA_OUT_LINK_CH3: usize = 0x504;
const DMA_OUT_INT_RAW_CH3: usize = 0x50C;
const DMA_OUT_INT_CLR_CH3: usize = 0x510;
const DMA_OUT_INT_ENA_CH3: usize = 0x514;

// ----------------------------------------------------------------------------
//  Helpers
// ----------------------------------------------------------------------------
fn reg_write(base: *mut u32, offset: usize, val: u32) {
    unsafe { write_volatile(base.add(offset / 4), val); }
}
fn reg_modify(base: *mut u32, offset: usize, and_mask: u32, or_mask: u32) {
    let val = unsafe { read_volatile(base.add(offset / 4)) };
    let new = (val & and_mask) | or_mask;
    unsafe { write_volatile(base.add(offset / 4), new); }
}
fn reg_read(base: *mut u32, offset: usize) -> u32 {
    unsafe { read_volatile(base.add(offset / 4)) }
}

// ----------------------------------------------------------------------------
//  DMA Descriptor
// ----------------------------------------------------------------------------
#[repr(C, align(16))]
pub struct DmaDescriptor {
    pub buffer: *mut u8,
    pub len: u32,
    pub size: u32,
    pub owner: u32,
    pub next: *mut DmaDescriptor,
}
impl DmaDescriptor {
    pub const fn empty() -> Self {
        Self { buffer: core::ptr::null_mut(), len: 0, size: 0, owner: 0, next: core::ptr::null_mut() }
    }
    pub fn set_owner_dma(&mut self) { self.owner |= 1 << 31; }
    pub fn set_suc_eof(&mut self) { self.owner |= 1 << 30; }
}

// ----------------------------------------------------------------------------
//  DMA-safe buffers
// ----------------------------------------------------------------------------
#[unsafe(link_section = ".dma")]
static mut RX_DESC: DmaDescriptor = DmaDescriptor::empty();
#[unsafe(link_section = ".dma")]
static mut TX_DESC: DmaDescriptor = DmaDescriptor::empty();
#[unsafe(link_section = ".dma")]
static mut RX_BUF: [u8; 4096] = [0; 4096];
#[unsafe(link_section = ".dma")]
static mut TX_BUF: [u8; 4096] = [0; 4096];

// ----------------------------------------------------------------------------
//  I2sChannel
// ----------------------------------------------------------------------------
pub struct I2sChannel {
    rx_desc: *mut DmaDescriptor,
    tx_desc: *mut DmaDescriptor,
    rx_buffer: *mut [u8; 4096],
    tx_buffer: *mut [u8; 4096],
}

impl I2sChannel {
    pub fn new_std(rx: bool, tx: bool, sample_rate_hz: u32, bits_per_sample: u8, mono: bool) -> Result<Self, EspError> {
        let rx_desc = addr_of_mut!(RX_DESC);
        let tx_desc = addr_of_mut!(TX_DESC);
        let rx_buffer = addr_of_mut!(RX_BUF);
        let tx_buffer = addr_of_mut!(TX_BUF);

        unsafe {
            if rx {
                (*rx_desc).buffer = (*rx_buffer).as_mut_ptr();
                (*rx_desc).len = 4096;
                (*rx_desc).size = 4096;
                (*rx_desc).set_owner_dma();
                (*rx_desc).set_suc_eof();
                (*rx_desc).next = core::ptr::null_mut();
            }
            if tx {
                (*tx_desc).buffer = (*tx_buffer).as_mut_ptr();
                (*tx_desc).len = 0;
                (*tx_desc).size = 4096;
                (*tx_desc).set_owner_dma();
                (*tx_desc).set_suc_eof();
                (*tx_desc).next = core::ptr::null_mut();
            }
        }

        // Reset I2S
        reg_modify(I2S0_BASE, I2S_CONF0_REG, 0, (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4));
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !((1 << 1) | (1 << 2) | (1 << 3) | (1 << 4)), 0);

        // Master mode
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 5), 0);
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 6), 0);

        // ----- Clock configuration for 16 kHz sample rate -----
        // Hardcoded values: MCLK = 4.096 MHz, BCLK = 512 kHz, source = PLL_160M.
        // MCLK divider: 160 MHz / 4.096 MHz = 39.0625 = 39 + 1/16
        reg_write(I2S0_BASE, I2S_CLKM_CONF_REG,
            (0 << 8) |          // clk_sel = PLL_160M
            (39 << 16) |        // clkm_div_num = 39
            (16 << 24) |        // clkm_div_a = 16
            (1 << 28) |         // clkm_div_b = 1
            (1 << 0)            // clk_active = 1
        );
        // BCLK = MCLK / 8, using MCLK as source (clk_sel = 2)
        reg_write(I2S0_BASE, I2S_TX_CLKM_CONF_REG,
            (2 << 8) |          // tx_clk_sel = MCLK
            (8 << 16) |         // tx_clkm_div_num = 8
            (1 << 24) |         // tx_clkm_div_a = 1
            (0 << 28) |         // tx_clkm_div_b = 0
            (1 << 0)            // tx_clk_active = 1
        );
        reg_write(I2S0_BASE, I2S_RX_CLKM_CONF_REG,
            (2 << 8) | (8 << 16) | (1 << 24) | (0 << 28) | (1 << 0)
        );

        // Data format
        let mono_bit = if mono { 1 } else { 0 };
        let bit_mode = if bits_per_sample > 16 { 1 } else { 0 };
        reg_modify(I2S0_BASE, I2S_RX_CONF_REG, 0, (mono_bit << 10) | (bit_mode << 4));
        reg_modify(I2S0_BASE, I2S_TX_CONF_REG, 0, (mono_bit << 10) | (bit_mode << 4));
        if bits_per_sample == 24 {
            reg_modify(I2S0_BASE, I2S_RX_CONF_REG, 0, 1 << 5);
            reg_modify(I2S0_BASE, I2S_TX_CONF_REG, 0, 1 << 5);
        }
        reg_modify(I2S0_BASE, I2S_RX_CONF1_REG, 0, 1 << 0);
        reg_modify(I2S0_BASE, I2S_TX_CONF1_REG, 0, 1 << 0);
        reg_modify(I2S0_BASE, I2S_FIFO_CONF_REG, 0, 1 << 1);

        if rx { reg_write(I2S0_BASE, I2S_RX_EOF_DES_ADDR_REG, rx_desc as u32); }
        if tx { reg_write(I2S0_BASE, I2S_TX_EOF_DES_ADDR_REG, tx_desc as u32); }

        // DMA channels 2 (RX) and 3 (TX)
        if rx {
            reg_modify(DMA_BASE, DMA_IN_CONF0_CH2, 0, 1 << 0);
            reg_modify(DMA_BASE, DMA_IN_CONF0_CH2, !(1 << 0), 0);
            reg_write(DMA_BASE, DMA_IN_LINK_CH2, rx_desc as u32);
            reg_modify(DMA_BASE, DMA_IN_CONF0_CH2, 0, (1 << 1) | (1 << 2) | (1 << 3) | (2 << 4));
            reg_modify(DMA_BASE, DMA_IN_INT_ENA_CH2, 0, 1 << 0);
        }
        if tx {
            reg_modify(DMA_BASE, DMA_OUT_CONF0_CH3, 0, 1 << 0);
            reg_modify(DMA_BASE, DMA_OUT_CONF0_CH3, !(1 << 0), 0);
            reg_write(DMA_BASE, DMA_OUT_LINK_CH3, tx_desc as u32);
            reg_modify(DMA_BASE, DMA_OUT_CONF0_CH3, 0, (1 << 1) | (1 << 2) | (1 << 3) | (2 << 4));
            reg_modify(DMA_BASE, DMA_OUT_INT_ENA_CH3, 0, 1 << 0);
        }

        Ok(Self { rx_desc, tx_desc, rx_buffer, tx_buffer })
    }

    // The rest of the methods (rx_enable, rx_disable, tx_enable, tx_disable, read, write)
    // are unchanged. Keep them from your previous version.
    // I'll include them below for completeness.
    
    pub fn rx_enable(&mut self) -> Result<(), EspError> {
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 4), 0);
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 4), 0);
        reg_modify(DMA_BASE, DMA_IN_LINK_CH2, 0, 1 << 0);
        reg_modify(I2S0_BASE, I2S_CONF0_REG, 0, 1 << 0);
        Ok(())
    }
    pub fn rx_disable(&mut self) -> Result<(), EspError> {
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 0), 0);
        reg_modify(DMA_BASE, DMA_IN_LINK_CH2, 0, 1 << 1);
        Ok(())
    }
    pub fn tx_enable(&mut self) -> Result<(), EspError> {
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 3), 0);
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 3), 0);
        reg_modify(DMA_BASE, DMA_OUT_LINK_CH3, 0, 1 << 0);
        reg_modify(I2S0_BASE, I2S_CONF0_REG, 0, 1 << 2);
        Ok(())
    }
    pub fn tx_disable(&mut self) -> Result<(), EspError> {
        reg_modify(I2S0_BASE, I2S_CONF0_REG, !(1 << 2), 0);
        reg_modify(DMA_BASE, DMA_OUT_LINK_CH3, 0, 1 << 1);
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8], timeout_ms: TickType_t) -> Result<usize, EspError> {
        let start = get_time_ms();
        while (reg_read(DMA_BASE, DMA_IN_INT_RAW_CH2) & 1) == 0 {
            if timeout_ms != 0 && get_time_ms() - start > timeout_ms {
                return Err(EspError::ErrTimeout);
            }
        }
        reg_modify(DMA_BASE, DMA_IN_INT_CLR_CH2, !(1 << 0), 0);
        let len = unsafe { (*self.rx_desc).len as usize };
        let copy_len = len.min(buf.len());
        unsafe {
            let src = (*self.rx_buffer).as_ptr();
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), copy_len);
        }
        unsafe { (*self.rx_desc).set_owner_dma(); }
        reg_modify(DMA_BASE, DMA_IN_LINK_CH2, 0, 1 << 0);
        Ok(copy_len)
    }

    pub fn write(&mut self, data: &[u8], timeout_ms: TickType_t) -> Result<usize, EspError> {
        let start = get_time_ms();
        while (reg_read(DMA_BASE, DMA_OUT_INT_RAW_CH3) & 1) == 0 {
            if timeout_ms != 0 && get_time_ms() - start > timeout_ms {
                return Err(EspError::ErrTimeout);
            }
        }
        reg_modify(DMA_BASE, DMA_OUT_INT_CLR_CH3, !(1 << 0), 0);
        let copy_len = data.len().min(4096);
        unsafe {
            let dst = (*self.tx_buffer).as_mut_ptr();
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, copy_len);
            (*self.tx_desc).len = copy_len as u32;
            (*self.tx_desc).set_owner_dma();
        }
        reg_modify(DMA_BASE, DMA_OUT_LINK_CH3, 0, 1 << 0);
        Ok(copy_len)
    }
}

// ----------------------------------------------------------------------------
//  Time helper
// ----------------------------------------------------------------------------
fn get_time_ms() -> u32 {
    static mut COUNT: u32 = 0;
    unsafe { COUNT += 1; COUNT }
}

// ----------------------------------------------------------------------------
//  GPIO setup (using PAC for GPIO base)
// ----------------------------------------------------------------------------
use esp32s3::GPIO;
fn set_i2s_pin(pin: u32, signal: u32, is_output: bool) {
    let gpio = unsafe { &*GPIO::ptr() };
    if is_output {
        let reg = gpio.func_out_sel_cfg(pin as usize).as_ptr();
        unsafe {
            let mut val = read_volatile(reg);
            val = (val & !0xFF) | (signal & 0xFF);
            val |= 1 << 9; // oe
            write_volatile(reg, val);
        }
    } else {
        let reg = gpio.func_in_sel_cfg(pin as usize).as_ptr();
        unsafe {
            let mut val = read_volatile(reg);
            val = (val & !0x1F) | (signal & 0x1F);
            write_volatile(reg, val);
        }
    }
}

// ----------------------------------------------------------------------------
//  I2sDriver, handles, traits (unchanged from your previous version)
// ----------------------------------------------------------------------------
use core::marker::PhantomData;
pub struct I2sDriver<'d, Dir> { channel: Option<I2sChannel>, _p: PhantomData<&'d ()>, _dir: PhantomData<Dir> }
pub struct I2sBiDir; pub struct I2sRx; pub struct I2sTx;
pub trait I2sRxSupported {} pub trait I2sTxSupported {}
impl I2sRxSupported for I2sRx {} impl I2sRxSupported for I2sBiDir {}
impl I2sTxSupported for I2sTx {} impl I2sTxSupported for I2sBiDir {}

impl<'d, Dir> I2sDriver<'d, Dir> {
    fn new_std_internal(rx: bool, tx: bool, sample_rate: u32, bits: u8, mono: bool,
        bclk: u32, ws: u32, din: Option<u32>, dout: Option<u32>, mclk: Option<u32>) -> Result<Self, EspError> {
        set_i2s_pin(bclk, 0x18, true);
        set_i2s_pin(ws, 0x19, true);
        set_i2s_pin(bclk, 0x18, false);
        set_i2s_pin(ws, 0x19, false);
        if let Some(p) = din { set_i2s_pin(p, 0x1A, false); }
        if let Some(p) = dout { set_i2s_pin(p, 0x1A, true); }
        if let Some(p) = mclk { set_i2s_pin(p, 0x1B, true); }
        let channel = I2sChannel::new_std(rx, tx, sample_rate, bits, mono)?;
        Ok(I2sDriver { channel: Some(channel), _p: PhantomData, _dir: PhantomData })
    }
}

impl<Dir: I2sRxSupported> I2sDriver<'_, Dir> {
    pub fn rx_enable(&mut self) -> Result<(), EspError> { self.channel.as_mut().unwrap().rx_enable() }
    pub fn rx_disable(&mut self) -> Result<(), EspError> { self.channel.as_mut().unwrap().rx_disable() }
    pub fn read(&mut self, buf: &mut [u8], timeout: TickType_t) -> Result<usize, EspError> {
        self.channel.as_mut().unwrap().read(buf, timeout)
    }
}
impl<Dir: I2sTxSupported> I2sDriver<'_, Dir> {
    pub fn tx_enable(&mut self) -> Result<(), EspError> { self.channel.as_mut().unwrap().tx_enable() }
    pub fn tx_disable(&mut self) -> Result<(), EspError> { self.channel.as_mut().unwrap().tx_disable() }
    pub fn write(&mut self, data: &[u8], timeout: TickType_t) -> Result<usize, EspError> {
        self.channel.as_mut().unwrap().write(data, timeout)
    }
}

impl I2sDriver<'_, I2sBiDir> {
    pub fn new_std_bidir(sample_rate: u32, bits: u8, bclk: u32, ws: u32, din: u32, dout: u32, mclk: Option<u32>) -> Result<Self, EspError> {
        Self::new_std_internal(true, true, sample_rate, bits, false, bclk, ws, Some(din), Some(dout), mclk)
    }
}
impl I2sDriver<'_, I2sRx> {
    pub fn new_std_rx(sample_rate: u32, bits: u8, bclk: u32, ws: u32, din: u32, mclk: Option<u32>) -> Result<Self, EspError> {
        Self::new_std_internal(true, false, sample_rate, bits, false, bclk, ws, Some(din), None, mclk)
    }
}
impl I2sDriver<'_, I2sTx> {
    pub fn new_std_tx(sample_rate: u32, bits: u8, bclk: u32, ws: u32, dout: u32, mclk: Option<u32>) -> Result<Self, EspError> {
        Self::new_std_internal(false, true, sample_rate, bits, false, bclk, ws, None, Some(dout), mclk)
    }
}
impl<Dir> Drop for I2sDriver<'_, Dir> { fn drop(&mut self) { self.channel.take(); } }

// ----------------------------------------------------------------------------
//  Split handles
// ----------------------------------------------------------------------------
pub struct I2sTxHandle { channel: *mut I2sChannel }
unsafe impl Send for I2sTxHandle {}
impl I2sTxHandle {
    pub fn write(&mut self, data: &[u8], timeout_ms: TickType_t) -> Result<usize, EspError> {
        unsafe { (*self.channel).write(data, timeout_ms) }
    }
}
pub struct I2sRxHandle { channel: *mut I2sChannel }
unsafe impl Send for I2sRxHandle {}
impl I2sRxHandle {
    pub fn read(&mut self, buf: &mut [u8], timeout_ms: TickType_t) -> Result<usize, EspError> {
        unsafe { (*self.channel).read(buf, timeout_ms) }
    }
}
impl<'d> I2sDriver<'d, I2sBiDir> {
    pub fn split(&mut self) -> (I2sTxHandle, I2sRxHandle) {
        let channel = self.channel.as_mut().unwrap() as *mut I2sChannel;
        (I2sTxHandle { channel }, I2sRxHandle { channel })
    }
}
