// COMPONENTS/TOUCH

use core::{marker::PhantomData, str};
use embedded_hal::i2c;

#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
    pub fingers: u8,
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum Gesture {
    None,
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
    SingleTap,
    DoubleTap,
    LongPress,
    Unknown(u8),
}

// DETECTED SWIPE GESTURE WITH START/END COORDINATES
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct SwipeEvent {
    pub direction: SwipeDirection,
    pub start_x: u16,
    pub start_y: u16,
    pub end_x: u16,
    pub end_y: u16,
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}





const GT911_I2C_ADDR_BA: u8 = 0x5D;
const GT911_PRODUCT_ID_REG: u16 = 0x8140;
const GT911_TOUCHPOINT_STATUS_REG: u16 = 0x814E;
const GT911_TOUCHPOINT_1_REG: u16 = 0x814F;
const GT911_COMMAND_REG: u16 = 0x8040;

const MAX_NUM_TOUCHPOINTS: usize = 5;
const TOUCHPOINT_ENTRY_LEN: usize = 8;
pub const GET_TOUCH_BUF_SIZE: usize = TOUCHPOINT_ENTRY_LEN;
pub const GET_MULTITOUCH_BUF_SIZE: usize = TOUCHPOINT_ENTRY_LEN * MAX_NUM_TOUCHPOINTS;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    pub track_id: u8,
    pub x: u16,
    pub y: u16,
    pub area: u16,
}


#[derive(Debug, Clone)]
pub enum Error<E> {
    UnexpectedProductId,
    I2C(E),
    NotReady,
}


pub struct Gt911Blocking<I2C> {
    i2c_addr: u8,
    i2c: PhantomData<I2C>,
}


impl<I2C> Default for Gt911Blocking<I2C> {
    fn default() -> Self {
        Self {
            i2c_addr: GT911_I2C_ADDR_BA,
            i2c: PhantomData,
        }
    }
}


impl<I2C, E> Gt911Blocking<I2C>
where
    I2C: embedded_hal::i2c::I2c<Error = E>,
{
    pub fn new(i2c_addr: u8) -> Self {
        Self {
            i2c_addr,
            i2c: PhantomData,
        }
    }

    pub fn init(&self, i2c: &mut I2C) -> Result<(), Error<E>> {
        self.write(i2c, GT911_COMMAND_REG, 0)?;

        let mut read = [0u8; 4];
        self.read(i2c, GT911_PRODUCT_ID_REG, &mut read)?;
        match str::from_utf8(&read) {
            Ok(product_id) => {
                if product_id != "911\0" {
                    return Err(Error::UnexpectedProductId);
                }
            }
            Err(_) => {
                return Err(Error::UnexpectedProductId);
            }
        }

        self.write(i2c, GT911_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(())
    }


    pub fn get_touch(&self, i2c: &mut I2C) -> Result<Option<Point>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c)?;

        let point = if num_touch_points > 0 {
            let mut read = [0u8; TOUCHPOINT_ENTRY_LEN];
            self.read(i2c, GT911_TOUCHPOINT_1_REG, &mut read)?;
            let point = decode_point(&read);
            Some(point)
        } else {
            None
        };

        self.write(i2c, GT911_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(point)
    }


    pub fn get_multi_touch(
        &self,
        i2c: &mut I2C,
    ) -> Result<heapless::Vec<Point, MAX_NUM_TOUCHPOINTS>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c)?;

        let points = if num_touch_points > 0 {
            assert!(num_touch_points <= MAX_NUM_TOUCHPOINTS);
            let mut points = heapless::Vec::new();

            let mut read = [0u8; TOUCHPOINT_ENTRY_LEN * MAX_NUM_TOUCHPOINTS];
            self.read(
                i2c,
                GT911_TOUCHPOINT_1_REG,
                &mut read[..TOUCHPOINT_ENTRY_LEN * num_touch_points],
            )?;

            for n in 0..num_touch_points {
                let start = n * TOUCHPOINT_ENTRY_LEN;
                let point = decode_point(&read[start..start + TOUCHPOINT_ENTRY_LEN]);
                points.push(point).ok();
            }

            points
        } else {
            heapless::Vec::new()
        };

        self.write(i2c, GT911_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(points)
    }

    fn get_num_touch_points(&self, i2c: &mut I2C) -> Result<usize, Error<E>> {
        let mut read = [0u8; 1];
        self.read(i2c, GT911_TOUCHPOINT_STATUS_REG, &mut read)?;

        let status = read[0];
        let ready = (status & 0x80) > 0;
        let num_touch_points = (status & 0x0F) as usize;

        if ready {
            Ok(num_touch_points)
        } else {
            Err(Error::NotReady)
        }
    }

    fn write(&self, i2c: &mut I2C, register: u16, value: u8) -> Result<(), Error<E>> {
        let register = register.to_be_bytes();
        let cmd = [register[0], register[1], value];
        i2c.write(self.i2c_addr, &cmd).map_err(Error::I2C)
    }

    fn read(&self, i2c: &mut I2C, register: u16, buf: &mut [u8]) -> Result<(), Error<E>> {
        i2c.write_read(self.i2c_addr, &register.to_be_bytes(), buf)
            .map_err(Error::I2C)
    }
}


fn decode_point(buf: &[u8]) -> Point {
    assert!(buf.len() >= TOUCHPOINT_ENTRY_LEN);
    Point {
        track_id: buf[0],
        x: u16::from_le_bytes([buf[1], buf[2]]),
        y: u16::from_le_bytes([buf[3], buf[4]]),
        area: u16::from_le_bytes([buf[5], buf[6]]),
    }
}
