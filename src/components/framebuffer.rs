// COMPONENTS/FRAMEBUFFER

use embedded_graphics_core::{
    geometry::{OriginDimensions, Size, Point},
    pixelcolor::Rgb565,
    primitives::Rectangle,
    Pixel,
    draw_target::DrawTarget,
};
use embedded_graphics_core::pixelcolor::IntoStorage;
use embedded_graphics_core::pixelcolor::raw::RawU16;

const WIDTH: usize = crate::state::LCD_WIDTH as usize;
const HEIGHT: usize = crate::state::LCD_HEIGHT as usize;
const PIXEL_COUNT: usize = WIDTH * HEIGHT;

pub struct Framebuffer {
    buf: alloc::vec::Vec<u16>,
    back: alloc::vec::Vec<u16>,
}

impl Framebuffer {
    /// ALLOCATE BOTH BUFFERS IN PSRAM
    pub fn new() -> Self {
        let buf = alloc::vec![0u16; PIXEL_COUNT];
        let back = alloc::vec![0u16; PIXEL_COUNT];
        Self { buf, back }
    }

    // SWAP FRONT & BACK BUFFERS 
    pub fn swap(&mut self) {
        core::mem::swap(&mut self.buf, &mut self.back);
    }

    // GET MUTABLE REF TO THE BACK BUFFER FOR OFF-SCREEN DRAWING
    pub fn back_buffer_mut(&mut self) -> &mut [u16] {
        &mut self.back
    }

    pub fn buffer_mut(&mut self) -> &mut [u16] {
        &mut self.buf
    }

    pub fn buffer(&self) -> &[u16] {
        &self.buf
    }

    pub fn clear_color(&mut self, color: Rgb565) {
        let raw: u16 = color.into_storage();
        self.buf.fill(raw);
    }


    #[inline(always)]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u16) {
        if x < WIDTH && y < HEIGHT {
            self.buf[y * WIDTH + x] = color;
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        let x_end = (x + w).min(WIDTH);
        let y_end = (y + h).min(HEIGHT);
        for row in y..y_end {
            let start = row * WIDTH + x;
            let end = row * WIDTH + x_end;
            self.buf[start..end].fill(color);
        }
    }

    pub fn flush_region<D>(
        &self,
        display: &mut D,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) where
        D: DrawTarget<Color = Rgb565>,
    {
        let x = x.min(WIDTH);
        let y = y.min(HEIGHT);
        let width = width.min(WIDTH - x);
        let height = height.min(HEIGHT - y);

        if width == 0 || height == 0 {
            return;
        }

        let area = Rectangle::new(
            Point::new(x as i32, y as i32),
            Size::new(width as u32, height as u32),
        );

        let colors = (y..y + height).flat_map(|row| {
            let start = row * WIDTH + x;
            let end = start + width;
            self.buf[start..end]
                .iter()
                .map(|&raw| Rgb565::from(RawU16::new(raw)))   // corrected conversion
        });

        let _ = display.fill_contiguous(&area, colors);
    }

    pub fn flush<D>(&self, display: &mut D)
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.flush_region(display, 0, 0, WIDTH, HEIGHT);
    }
}


impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0
                && coord.x < WIDTH as i32
                && coord.y >= 0
                && coord.y < HEIGHT as i32
            {
                let raw: u16 = color.into_storage();
                self.buf[coord.y as usize * WIDTH + coord.x as usize] = raw;
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(
        &mut self,
        area: &Rectangle,
        colors: I,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(WIDTH as u32, HEIGHT as u32),
        ));
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let x = area.top_left.x as usize;
        let y = area.top_left.y as usize;
        let w = area.size.width as usize;
        let mut row = y;
        let mut col = 0;

        for color in colors {
            if col < w && row < HEIGHT {
                self.buf[row * WIDTH + x + col] = color.into_storage();
            }
            col += 1;
            if col >= w {
                col = 0;
                row += 1;
            }
        }
        Ok(())
    }

    fn fill_solid(
        &mut self,
        area: &Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(WIDTH as u32, HEIGHT as u32),
        ));
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }
        let raw: u16 = color.into_storage();
        self.fill_rect(
            area.top_left.x as usize,
            area.top_left.y as usize,
            area.size.width as usize,
            area.size.height as usize,
            raw,
        );
        Ok(())
    }
}
