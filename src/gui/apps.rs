// GUI/APPS
// BIG APP LAUNCHER:
// 1x1 APP ICON GRID, SWIPE UP/DOWN FOR SMOOTH SCROLLING TRANSITIONS BETWEEN APPLICATIONS

// ───────────────────────────────────────────────────────────────────────
// TRAITS
use embedded_graphics::Drawable;
use embedded_graphics::prelude::Point;
use embedded_graphics::text::{Text, TextStyleBuilder, Alignment};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics_core::pixelcolor::IntoStorage;
use alloc::vec;
use alloc::vec::Vec;

// ───────────────────────────────────────────────────────────────────────
// CONSTANTS
const PAGE_HEIGHT: i32 = crate::state::LCD_HEIGHT as i32;  // 502

// ───────────────────────────────────────────────────────────────────────
// LAUNCHER STATE
pub struct Launcher {
    pub scroll_offset: i32,
    pub target_scroll: i32,
}

pub(crate) static LAUNCHER: critical_section::Mutex<core::cell::RefCell<Launcher>> =
    critical_section::Mutex::new(core::cell::RefCell::new(Launcher {
        scroll_offset: 0,
        target_scroll: 0,
    }));

// ───────────────────────────────────────────────────────────────────────
// SLICE DRAW TARGET (USED ONLY DURING PRE‑RENDERING)
struct SliceDrawTarget<'a> {
    buf: &'a mut [u16],
    width: usize,
    height: usize,
}

impl<'a> embedded_graphics_core::draw_target::DrawTarget for SliceDrawTarget<'a> {
    type Color = embedded_graphics::pixelcolor::Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Self::Color>>,
    {
        for embedded_graphics_core::Pixel(coord, color) in pixels.into_iter() {
            let x = coord.x as usize;
            let y = coord.y as usize;
            if x < self.width && y < self.height {
                let raw: u16 = color.into_storage();
                self.buf[y * self.width + x] = raw;
            }
        }
        Ok(())
    }
}

impl<'a> embedded_graphics_core::geometry::OriginDimensions for SliceDrawTarget<'a> {
    fn size(&self) -> embedded_graphics_core::geometry::Size {
        embedded_graphics_core::geometry::Size::new(self.width as u32, self.height as u32)
    }
}

// ───────────────────────────────────────────────────────────────────────
// PAGE CACHE (PRE‑RENDERED FULL‑SCREEN IMAGES)
struct PageBufferCache {
    // TWO REUSABLE FULL-SCREEN BUFFERS
    buffers: [Vec<u16>; 2],
    // APP INDEX THAT EACH BUFFER CURRENTLY REPRESENTS, -1 IF INVALID
    indices: [i32; 2],
    evict_slot: usize,
}

static PAGE_BUFFERS: critical_section::Mutex<core::cell::RefCell<Option<PageBufferCache>>> =
    critical_section::Mutex::new(core::cell::RefCell::new(None));

// RENDER SINGLE APP PAGE INTO AN EXISTING SLICE
fn render_page_into(app_idx: usize, buf: &mut [u16]) {
    let screen_w = crate::state::LCD_WIDTH as usize;
    let screen_h = crate::state::LCD_HEIGHT as usize;
    buf.fill(0x0000);

    let app = &crate::applications::APPS[app_idx];

    // DRAW APP ICON
    if let Ok(png) = embedded_png::Png::load_from_bytes(app.icon) {
        let icon_w = png.width() as i32;
        let icon_h = png.height() as i32;
        let target_h = (screen_h as f32 * 0.9) as i32;
        let scale = core::cmp::max(1, target_h / icon_h.max(1));
        let scaled_w = icon_w * scale;
        let scaled_h = icon_h * scale;
        let x = (screen_w as i32 - scaled_w) / 2;
        let y = (screen_h as i32 - scaled_h) / 2;

        for sy in 0..icon_h {
            for sx in 0..icon_w {
                if let Some(color) = png.pixels()[(sy * png.width() as i32 + sx) as usize] {
                    let raw: u16 = color.into_storage();
                    let px = x + sx * scale;
                    let py = y + sy * scale;
                    for dy in 0..scale {
                        let row = (py + dy) as usize;
                        if row >= screen_h { break; }
                        for dx in 0..scale {
                            let col = (px + dx) as usize;
                            if col < screen_w {
                                buf[row * screen_w + col] = raw;
                            }
                        }
                    }
                }
            }
        }
    }

    // APP NAME
    let mut slice_dt = SliceDrawTarget {
        buf,
        width: screen_w,
        height: screen_h,
    };
    let name_font = MonoTextStyle::new(
        &embedded_graphics::mono_font::ascii::FONT_10X20,
        crate::gui::colors::WHITE,
    );
    let name_align = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .build();
    let name_y = screen_h as i32 * 4 / 5;
    let _ = Text::with_text_style(
        app.name,
        Point::new(screen_w as i32 / 2, name_y),
        name_font,
        name_align,
    )
    .draw(&mut slice_dt);
}


// ───────────────────────────────────────────────────────────────────────
// INPUT HANDLERS
// SWIPE UP === NEXT APP, SWIPE DOWN === PREVIOUS APP.
pub fn handle_swipe(dir: crate::components::gt911::SwipeDirection) {
    let total = crate::applications::APPS.len();
    if total == 0 { return; }
    let max_scroll = (total - 1) as i32 * PAGE_HEIGHT;
    critical_section::with(|cs| {
        let mut launcher = LAUNCHER.borrow_ref_mut(cs);
        match dir {
            crate::components::gt911::SwipeDirection::Up => {
                launcher.target_scroll = (launcher.target_scroll + PAGE_HEIGHT).min(max_scroll);
            }
            crate::components::gt911::SwipeDirection::Down => {
                launcher.target_scroll = (launcher.target_scroll - PAGE_HEIGHT).max(0);
            }
            _ => {}
        }
    });
}

// SINGLE TAP DOES NOTHING - AVOIDS UNWANTED ACTIONS WHEN SCROLLING
pub fn handle_tap() {
    defmt::info!("👆");    
}

// DOUBLE-TAP ANYWHERE ON THE APPS PAGE > LAUNCH THE CURRENTLY DISPLAYED APP!
pub fn handle_double_tap(_x: u16, _y: u16) {
    defmt::info!("👆👆");
    let total = crate::applications::APPS.len();
    if total == 0 { return; }
    let idx = critical_section::with(|cs| {
        let launcher = LAUNCHER.borrow_ref(cs);
        (launcher.scroll_offset / PAGE_HEIGHT).clamp(0, (total - 1) as i32) as usize
    });
    let app = &crate::applications::APPS[idx];
    (app.launch)();
}

// ───────────────────────────────────────────────────────────────────────
// FRAME COMPOSITION (SCROLLING VIEW)
// COMPOSE THE CURRENT FRAME INTO `buf` USING THE CACHED PAGES.
// `scroll_offset` IS THE CURRENT SMOOTH‑ANIMATED OFFSET IN PIXELS.
pub fn compose(buf: &mut [u16], scroll_offset: i32) {
    let total = crate::applications::APPS.len();
    if total == 0 {
        buf.fill(0x0000);
        return;
    }

    let screen_w = crate::state::LCD_WIDTH as usize;
    let screen_h = crate::state::LCD_HEIGHT as usize;
    let page_h = screen_h as i32;

    buf.fill(0x0000);

    let current_page = scroll_offset / page_h;
    let progress = scroll_offset % page_h;

    critical_section::with(|cs| {
        let mut cache_opt = PAGE_BUFFERS.borrow_ref_mut(cs);
        if cache_opt.is_none() {
            let screen_pixels = screen_w * screen_h;
            *cache_opt = Some(PageBufferCache {
                buffers: [vec![0u16; screen_pixels], vec![0u16; screen_pixels]],
                indices: [-1, -1],
                evict_slot: 0,
            });
        }
        let cache = cache_opt.as_mut().unwrap();

        // HELPER 2 GET A RENDERED CACHED PAGE 
        fn ensure_page<'a>(cache: &'a mut PageBufferCache, idx: i32) -> Option<&'a [u16]> {
            if idx < 0 || idx >= crate::applications::APPS.len() as i32 {
                return None;
            }
            for i in 0..2 {
                if cache.indices[i] == idx {
                    return Some(&cache.buffers[i]);
                }
            }
            let slot = cache.evict_slot;
            render_page_into(idx as usize, &mut cache.buffers[slot]);
            cache.indices[slot] = idx;
            cache.evict_slot = (slot + 1) % 2;
            Some(&cache.buffers[slot])
        }

        // ROW COPY HELPER 
        fn copy_rows(
            dest: &mut [u16],
            src: &[u16],
            src_y_start: usize,
            height: usize,
            dest_y_offset: usize,
            screen_w: usize,
            screen_h: usize,
        ) {
            for row in 0..height {
                let src_row = src_y_start + row;
                let dest_row = dest_y_offset + row;
                if src_row >= screen_h || dest_row >= screen_h { break; }
                let src_begin = src_row * screen_w;
                let dest_begin = dest_row * screen_w;
                dest[dest_begin..dest_begin + screen_w]
                    .copy_from_slice(&src[src_begin..src_begin + screen_w]);
            }
        }

        // DRAW CURRENT
        if let Some(src) = ensure_page(cache, current_page) {
            copy_rows(buf, src, progress as usize, screen_h - progress as usize, 0, screen_w, screen_h);
        }

        // IF SCROLLING - DRAW NEXT PAGE TOO
        if progress > 0 {
            let next_page = current_page + 1;
            if let Some(src) = ensure_page(cache, next_page) {
                copy_rows(buf, src, 0, progress as usize, screen_h - progress as usize, screen_w, screen_h);
            }
        }
    });
}
