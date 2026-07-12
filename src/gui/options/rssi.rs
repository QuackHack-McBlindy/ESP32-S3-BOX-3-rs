// GUI/OPTIONS/RSSI
// DISPLAY WIFI SIGNAL STRENGTH
// + SCAN FOR WIFI NETWORKS
// VIEW IS SPLITTED WHEN RESULTS ARRIVE

use embedded_graphics::Drawable;
use embedded_graphics::prelude::Primitive;

// ───────────────────────────────────────────────────────────────────────
// CONSTANTS
const SCAN_LIST_LINE_HEIGHT: i32 = 80;
const SCAN_LIST_MARGIN: i32 = 20;
const SCAN_LIST_BUFFER_HEIGHT: usize = 1000;

// ───────────────────────────────────────────────────────────────────────
// SPLIT ANIMATION STATE
pub struct RssiSplit {
    pub target_offset: i32,
    pub current_offset: i32,
}

pub(crate) static RSSI_SPLIT: critical_section::Mutex<core::cell::RefCell<RssiSplit>> =
    critical_section::Mutex::new(core::cell::RefCell::new(RssiSplit {
        target_offset: 0,
        current_offset: 0,
    }));

pub fn open_split() {
    critical_section::with(|cs| {
        let mut split = RSSI_SPLIT.borrow_ref_mut(cs);
        split.target_offset = crate::state::LCD_HEIGHT as i32 / 2;
    });
}

pub fn close_split() {
    critical_section::with(|cs| {
        let mut split = RSSI_SPLIT.borrow_ref_mut(cs);
        split.target_offset = 0;
    });
}

pub fn animate_split(anim_speed: i32) {
    critical_section::with(|cs| {
        let mut split = RSSI_SPLIT.borrow_ref_mut(cs);
        let diff = split.target_offset - split.current_offset;
        if diff != 0 {
            let step = diff.clamp(-anim_speed, anim_speed);
            split.current_offset += step;
        }
    });
}

pub fn is_split_open() -> bool {
    critical_section::with(|cs| RSSI_SPLIT.borrow_ref(cs).current_offset > 0)
}

// ───────────────────────────────────────────────────────────────────────
// SCROLLING STATE
pub struct RssiScroll {
    pub offset: i32,
    pub target: i32,
    pub max_scroll: i32,
}

pub(crate) static RSSI_SCROLL: critical_section::Mutex<core::cell::RefCell<RssiScroll>> =
    critical_section::Mutex::new(core::cell::RefCell::new(RssiScroll {
        offset: 0,
        target: 0,
        max_scroll: 0,
    }));

pub fn animate_scroll(speed: i32) {
    critical_section::with(|cs| {
        let mut scroll = RSSI_SCROLL.borrow_ref_mut(cs);
        let diff = scroll.target - scroll.offset;
        if diff != 0 {
            let step = diff.clamp(-speed, speed);
            scroll.offset += step;
        }
    });
}

// ───────────────────────────────────────────────────────────────────────
// CACHED SCAN RESULTS
static SCAN_LIST_CACHE: critical_section::Mutex<core::cell::RefCell<core::option::Option<alloc::vec::Vec<u16>>>> =
    critical_section::Mutex::new(core::cell::RefCell::new(core::option::Option::None));
static SCAN_LIST_DIRTY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

// AFTER A NEW SCAN, WE MARK THE LIST DIRTY SO IT WILL BE RE‑RENDERED.
pub fn invalidate_scan_list() {
    SCAN_LIST_DIRTY.store(true, core::sync::atomic::Ordering::Release);
}

// BUILD (OR REBUILD) THE SCAN‑LIST PIXEL BUFFER.
fn render_scan_list_to_buffer(
    buf: &mut [u16],
    screen_w: usize,
    total_height: usize,
    results: &[esp_radio::wifi::ap::AccessPointInfo],
) -> i32 {
    buf[..screen_w * total_height].fill(0);

    let bold_font = critical_section::with(|_| unsafe {
        let ptr = core::ptr::addr_of!(crate::gui::ROBOTO_BOLD_FONT);
        (*ptr).as_ref().expect("FONT NOT INITIALISED").clone()
    });

    // ONLY CYAN COLOR – NO SIGNAL STRENGTH
    let cyan = crate::gui::colors::CYAN;

    let left_align = embedded_graphics::text::TextStyleBuilder::new()
        .alignment(embedded_graphics::text::Alignment::Left)
        .build();

    let mut target = RawBufferDrawTarget::new(buf, screen_w, total_height);
    let line_height = SCAN_LIST_LINE_HEIGHT;
    let margin = SCAN_LIST_MARGIN;
    let mut y = margin;

    for ap in results {
        if y + line_height > total_height as i32 {
            break;
        }

        let style = embedded_ttf::FontTextStyleBuilder::new(bold_font.clone())
            .font_size(60)
            .text_color(cyan)
            .build();

        let ssid = ap.ssid.as_str();
        let text = embedded_graphics::text::Text::with_text_style(
            ssid,
            embedded_graphics::prelude::Point::new(20, y),
            style,
            left_align,
        );
        embedded_graphics::prelude::Drawable::draw(&text, &mut target).ok();

        y += line_height;
    }

    y // TOTAL CONTENT HEIGHT
}

// ───────────────────────────────────────────────────────────────────────
// RAW BUFFER DRAW TARGET
struct RawBufferDrawTarget<'a> {
    buf: &'a mut [u16],
    width: usize,
    height: usize,
}

impl<'a> RawBufferDrawTarget<'a> {
    fn new(buf: &'a mut [u16], width: usize, height: usize) -> Self {
        Self { buf, width, height }
    }
}

impl embedded_graphics::prelude::OriginDimensions for RawBufferDrawTarget<'_> {
    fn size(&self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(self.width as u32, self.height as u32)
    }
}

impl embedded_graphics::draw_target::DrawTarget for RawBufferDrawTarget<'_> {
    type Color = embedded_graphics::pixelcolor::Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> core::result::Result<(), Self::Error>
    where
        I: core::iter::IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels.into_iter() {
            let point = pixel.0;
            if point.x >= 0 && point.x < self.width as i32 && point.y >= 0 && point.y < self.height as i32 {
                let idx = (point.y as usize) * self.width + (point.x as usize);
                self.buf[idx] = embedded_graphics::pixelcolor::IntoStorage::into_storage(pixel.1);
            }
        }
        core::result::Result::Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> core::result::Result<(), Self::Error> {
        let raw = embedded_graphics::pixelcolor::IntoStorage::into_storage(color);
        for item in self.buf.iter_mut() {
            *item = raw;
        }
        core::result::Result::Ok(())
    }
}

// ───────────────────────────────────────────────────────────────────────
// HIT AREA FOR THE SCAN
static mut HIT_AREA: core::option::Option<crate::gui::HitArea> = core::option::Option::None;

// ───────────────────────────────────────────────────────────────────────
// PUBLIC DRAW FUNCTION
pub fn draw(fb: &mut crate::components::framebuffer::Framebuffer) {
    // ANIMATE SPLIT AND SCROLL
    animate_split(8);
    animate_scroll(8);

    let offset = critical_section::with(|cs| RSSI_SPLIT.borrow_ref(cs).current_offset);
    let screen_w = crate::state::LCD_WIDTH as usize;
    let screen_h = crate::state::LCD_HEIGHT as usize;
    let w = screen_w as i32;
    let h = screen_h as i32;

    // ALWAYS CLEAR TO BLACK
    let _ = embedded_graphics::primitives::Rectangle::new(
        embedded_graphics::prelude::Point::zero(),
        embedded_graphics_core::geometry::Size::new(w as u32, h as u32),
    )
    .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(crate::gui::colors::BLACK))
    .draw(fb);

    // HEADER "RSSI"
    let bold_font = rusttype::Font::try_from_bytes(crate::base::assets::ROBOTO_BOLD).unwrap();
    let header_style = embedded_ttf::FontTextStyleBuilder::new(bold_font.clone())
        .font_size(86)
        .text_color(crate::gui::colors::CYAN)
        .build();
    let header_align = embedded_graphics::text::TextStyleBuilder::new()
        .alignment(embedded_graphics::text::Alignment::Center)
        .build();
    let _ = embedded_graphics::text::Text::with_text_style(
        "RSSI",
        embedded_graphics::prelude::Point::new(w / 2, 20),
        header_style,
        header_align,
    )
    .draw(fb);

    if offset == 0 {
        // CLOSED STATE: SHOW FULL GAUGE + SCAN BUTTON
        draw_signal_gauge(fb);
        draw_scan_button(fb);
    } else {
        // SPLIT OPEN: TOP HALF = SHRUNK GAUGE, BOTTOM = EMPTY, GAP = SCAN LIST
        let split_line_y = h / 2;

        // TOP HALF: DRAW A SMALLER VERSION OF THE GAUGE (ABOVE THE GAP)
        draw_signal_gauge_in_region(fb, 0, split_line_y - offset);

        // GAP: RENDER THE CACHED SCAN LIST
        if offset > 0 {
            let gap_top = split_line_y - offset;
            let gap_bottom = split_line_y + offset;
            let gap_height = (gap_bottom - gap_top).max(0) as usize;

            // REFRESH THE SCAN LIST CACHE IF DIRTY
            if SCAN_LIST_DIRTY.swap(false, core::sync::atomic::Ordering::AcqRel) {
                let results = {
                    let guard = crate::base::wifi::SCAN_RESULTS.try_lock().unwrap();
                    guard.clone()
                };
                let mut buf = alloc::vec![0u16; screen_w * SCAN_LIST_BUFFER_HEIGHT];
                let content_height = render_scan_list_to_buffer(
                    &mut buf,
                    screen_w,
                    SCAN_LIST_BUFFER_HEIGHT,
                    &results,
                );

                let max_scroll = (content_height - gap_height as i32).max(0);
                critical_section::with(|cs| {
                    let mut scroll = RSSI_SCROLL.borrow_ref_mut(cs);
                    scroll.max_scroll = max_scroll;
                    scroll.target = scroll.target.clamp(0, max_scroll);
                    scroll.offset = scroll.offset.clamp(0, max_scroll);
                    *SCAN_LIST_CACHE.borrow_ref_mut(cs) = core::option::Option::Some(buf);
                });
            }

            let list_buf_ptr = critical_section::with(|cs| {
                SCAN_LIST_CACHE.borrow_ref(cs).as_ref().unwrap().as_ptr()
            });

            let scroll_off = critical_section::with(|cs| {
                RSSI_SCROLL.borrow_ref(cs).offset.clamp(0, i32::MAX)
            }) as usize;

            let dest = fb.buffer_mut();
            let src_start = scroll_off * screen_w;
            // SAFETY: LIST_BUF_PTR POINTS TO A BUFFER OF SIZE SCREEN_W * SCAN_LIST_BUFFER_HEIGHT
            let src = unsafe {
                core::slice::from_raw_parts(list_buf_ptr.add(src_start), screen_w * gap_height)
            };
            let dst_start_row = gap_top as usize;
            for row in 0..gap_height {
                let dst_row = dst_start_row + row;
                if dst_row < screen_h {
                    let dst_start = dst_row * screen_w;
                    let src_start = row * screen_w;
                    dest[dst_start..dst_start + screen_w]
                        .copy_from_slice(&src[src_start..src_start + screen_w]);
                }
            }

            // TEAL GLOW LINES AT THE EDGES OF THE GAP
            draw_split_glow(fb, gap_top, gap_bottom, screen_w, screen_h);
        }

    }
}

// ───────────────────────────────────────────────────────────────────────
// HELPER: DRAW THE SIGNAL GAUGE INTO A SPECIFIC VERTICAL REGION [Y_MIN, Y_MAX]
fn draw_signal_gauge_in_region(fb: &mut crate::components::framebuffer::Framebuffer, y_min: i32, y_max: i32) {
    let region_height = y_max - y_min;
    if region_height <= 0 {
        return;
    }
    let is_on = crate::load!(crate::state::WIFI_CONNECTED);
    let rssi: i32 = crate::load!(crate::state::RSSI).into();
    let rssi_clamped = rssi.clamp(-90, -30);
    let percent: u8 = ((rssi_clamped + 90) * 100 / 60) as u8;
    let wifi_on: bool = crate::load!(crate::state::WIFI_STATE);

    let w = crate::state::LCD_WIDTH as i32;
    let h = crate::state::LCD_HEIGHT as i32;

    let min_dim = if w < h { w } else { h } as u32;
    let max_diameter = (region_height as u32 * 7) / 10;
    let diameter = min_dim.min(max_diameter) * 7 / 10;
    let center_x = w / 2;
    let center_y = y_min + region_height / 2;
    let top_left = embedded_graphics::prelude::Point::new(center_x - diameter as i32 / 2, center_y - diameter as i32 / 2);
    let stroke_width = 4u32;

    let bg_arc = embedded_graphics::primitives::Arc::new(
        top_left,
        diameter,
        embedded_graphics::geometry::Angle::from_degrees(270.0),
        embedded_graphics::geometry::Angle::from_degrees(360.0),
    );
    let _ = bg_arc
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                .stroke_color(crate::gui::colors::GRAY)
                .stroke_width(stroke_width)
                .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
                .build(),
        )
        .draw(fb);

    let fill_color = crate::gui::colors::gradient_red_green(percent);
    if percent > 0 {
        let sweep_deg = -360.0 * percent as f32 / 100.0;
        let fill_arc = embedded_graphics::primitives::Arc::new(
            top_left,
            diameter,
            embedded_graphics::geometry::Angle::from_degrees(270.0),
            embedded_graphics::geometry::Angle::from_degrees(sweep_deg),
        );
        let _ = fill_arc
            .into_styled(
                embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                    .stroke_color(fill_color)
                    .stroke_width(stroke_width)
                    .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
                    .build(),
            )
            .draw(fb);
    }

    // WI‑FI ICON IN THE MIDDLE (SCALED TO FIT)
    let icon_bytes = if wifi_on {
        crate::base::assets::SETTINGS_WIFI_ON_PNG
    } else { crate::base::assets::SETTINGS_WIFI_OFF_PNG };
    if let core::result::Result::Ok(icon_png) = embedded_png::Png::load_from_bytes(icon_bytes) {
        let img_w = icon_png.width() as i32;
        let img_h = icon_png.height() as i32;
        let max_icon_h = (diameter as f32 * 0.55) as i32;
        let scale = core::cmp::max(1, max_icon_h / img_h.max(1));
        let scaled_w = img_w * scale;
        let scaled_h = img_h * scale;
        let x = center_x - scaled_w / 2;
        let y = center_y - scaled_h / 2 - 15;
        let dest = fb.buffer_mut();
        let screen_w = w as usize;
        let screen_h = h as usize;
        for sy in 0..img_h {
            for sx in 0..img_w {
                let idx = (sy * img_w + sx) as usize;
                if let core::option::Option::Some(color) = icon_png.pixels()[idx] {
                    let raw: u16 = if is_on {
                        embedded_graphics::pixelcolor::IntoStorage::into_storage(color)
                    } else {
                        embedded_graphics::pixelcolor::IntoStorage::into_storage(crate::gui::colors::RED)
                    };
                    let px = x + sx * scale;
                    let py = y + sy * scale;
                    for dy in 0..scale {
                        let row = (py + dy) as usize;
                        if row >= screen_h { break; }
                        for dx in 0..scale {
                            let col = (px + dx) as usize;
                            if col < screen_w {
                                dest[row * screen_w + col] = raw;
                            }
                        }
                    }
                }
            }
        }
    }

    // DBM VALUE BELOW THE ICON
    let rssi_font = rusttype::Font::try_from_bytes(crate::base::assets::ROBOTO_BOLD).unwrap();
    let rssi_style = embedded_ttf::FontTextStyleBuilder::new(rssi_font)
        .font_size(42)
        .text_color(crate::gui::colors::WHITE)
        .build();
    let rssi_text = format_rssi(rssi);
    let rssi_align = embedded_graphics::text::TextStyleBuilder::new()
        .alignment(embedded_graphics::text::Alignment::Center)
        .build();
    let _ = embedded_graphics::text::Text::with_text_style(
        &rssi_text,
        embedded_graphics::prelude::Point::new(center_x, center_y + 30),
        rssi_style,
        rssi_align,
    )
    .draw(fb);
}

// ───────────────────────────────────────────────────────────────────────
// DRAW SIGNAL GAUGE (FULL SCREEN, CLOSED STATE)
fn draw_signal_gauge(fb: &mut crate::components::framebuffer::Framebuffer) {
    let is_on = crate::load!(crate::state::WIFI_CONNECTED);
    let rssi: i32 = crate::load!(crate::state::RSSI).into();
    let rssi_clamped = rssi.clamp(-90, -30);
    let percent: u8 = ((rssi_clamped + 90) * 100 / 60) as u8;
    let wifi_on: bool = crate::load!(crate::state::WIFI_STATE);

    let w = crate::state::LCD_WIDTH as i32;
    let h = crate::state::LCD_HEIGHT as i32;
    let min_dim = if w < h { w } else { h } as u32;
    let diameter = min_dim * 7 / 10;
    let center_x = w / 2;
    let center_y = h / 2;
    let top_left = embedded_graphics::prelude::Point::new(center_x - diameter as i32 / 2, center_y - diameter as i32 / 2);
    let stroke_width = 5u32;

    let bg_arc = embedded_graphics::primitives::Arc::new(
        top_left,
        diameter,
        embedded_graphics::geometry::Angle::from_degrees(270.0),
        embedded_graphics::geometry::Angle::from_degrees(360.0),
    );
    let _ = bg_arc
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                .stroke_color(crate::gui::colors::GRAY)
                .stroke_width(stroke_width)
                .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
                .build(),
        )
        .draw(fb);

    let fill_color = crate::gui::colors::gradient_red_green(percent);
    if percent > 0 {
        let sweep_deg = -360.0 * percent as f32 / 100.0;
        let fill_arc = embedded_graphics::primitives::Arc::new(
            top_left,
            diameter,
            embedded_graphics::geometry::Angle::from_degrees(270.0),
            embedded_graphics::geometry::Angle::from_degrees(sweep_deg),
        );
        let _ = fill_arc
            .into_styled(
                embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                    .stroke_color(fill_color)
                    .stroke_width(stroke_width)
                    .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
                    .build(),
            )
            .draw(fb);
    }

    let icon_bytes = if wifi_on {
        crate::base::assets::SETTINGS_WIFI_ON_PNG
    } else {
        crate::base::assets::SETTINGS_WIFI_OFF_PNG
    };
    if let core::result::Result::Ok(icon_png) = embedded_png::Png::load_from_bytes(icon_bytes) {
        let img_w = icon_png.width() as i32;
        let img_h = icon_png.height() as i32;
        let max_icon_h = (diameter as f32 * 0.66) as i32;
        let scale = core::cmp::max(1, max_icon_h / img_h.max(1));
        let scaled_w = img_w * scale;
        let scaled_h = img_h * scale;
        let x = center_x - scaled_w / 2;
        let y = center_y - scaled_h / 2 - 15;
        let dest = fb.buffer_mut();
        let screen_w = w as usize;
        let screen_h = h as usize;
        for sy in 0..img_h {
            for sx in 0..img_w {
                let idx = (sy * img_w + sx) as usize;
                if let core::option::Option::Some(color) = icon_png.pixels()[idx] {
                    let raw: u16 = if is_on {
                        embedded_graphics::pixelcolor::IntoStorage::into_storage(color)
                    } else {
                        embedded_graphics::pixelcolor::IntoStorage::into_storage(crate::gui::colors::RED)
                    };
                    let px = x + sx * scale;
                    let py = y + sy * scale;
                    for dy in 0..scale {
                        let row = (py + dy) as usize;
                        if row >= screen_h { break; }
                        for dx in 0..scale {
                            let col = (px + dx) as usize;
                            if col < screen_w {
                                dest[row * screen_w + col] = raw;
                            }
                        }
                    }
                }
            }
        }
    }

    let rssi_font = rusttype::Font::try_from_bytes(crate::base::assets::ROBOTO_BOLD).unwrap();
    let rssi_style = embedded_ttf::FontTextStyleBuilder::new(rssi_font)
        .font_size(82)
        .text_color(crate::gui::colors::WHITE)
        .build();
    let rssi_text = format_rssi(rssi);
    let rssi_align = embedded_graphics::text::TextStyleBuilder::new()
        .alignment(embedded_graphics::text::Alignment::Center)
        .build();
    let _ = embedded_graphics::text::Text::with_text_style(
        &rssi_text,
        embedded_graphics::prelude::Point::new(center_x, center_y + 70),
        rssi_style,
        rssi_align,
    )
    .draw(fb);
}

// ───────────────────────────────────────────────────────────────────────
// SCAN BUTTON (CLOSED STATE)
fn draw_scan_button(fb: &mut crate::components::framebuffer::Framebuffer) {
    let bold_font = rusttype::Font::try_from_bytes(crate::base::assets::ROBOTO_BOLD).unwrap();
    let style = embedded_ttf::FontTextStyleBuilder::new(bold_font)
        .font_size(68)
        .text_color(crate::gui::colors::CYAN)
        .build();
    let align = embedded_graphics::text::TextStyleBuilder::new()
        .alignment(embedded_graphics::text::Alignment::Center)
        .build();

    let text = "SCAN!";
    let text_w = 200;
    let text_h = 68;
    let w = crate::state::LCD_WIDTH as i32;
    let h = crate::state::LCD_HEIGHT as i32;
    let x = w / 2;
    let y = h - text_h - 20;

    let _ = embedded_graphics::text::Text::with_text_style(
        text,
        embedded_graphics::prelude::Point::new(x, y),
        style,
        align,
    )
    .draw(fb);

    let area = crate::gui::HitArea {
        x: x - text_w / 2,
        y,
        width: text_w as u32,
        height: text_h as u32,
        action: crate::gui::TouchAction::SettingsToggleWifiScan,
    };
    unsafe { HIT_AREA = core::option::Option::Some(area); }
}


// ───────────────────────────────────────────────────────────────────────
// DRAW TEAL GLOW LINES
fn draw_split_glow(
    fb: &mut crate::components::framebuffer::Framebuffer,
    gap_top: i32,
    gap_bottom: i32,
    screen_w: usize,
    screen_h: usize,
) {
    let teal_bright: u16 = 0x07FF;
    let teal_dark: u16 = 0x020F;
    let glow_half_thickness = 2i32;

    for i in -glow_half_thickness..=glow_half_thickness {
        let color = if i == 0 { teal_bright } else { teal_dark };
        let y1 = gap_top + i;
        let y2 = gap_bottom + i;
        if y1 >= 0 && (y1 as usize) < screen_h {
            let row_start = (y1 as usize) * screen_w;
            let dest = fb.buffer_mut();
            for x in 0..screen_w {
                dest[row_start + x] = color;
            }
        }
        if y2 >= 0 && (y2 as usize) < screen_h {
            let row_start = (y2 as usize) * screen_w;
            let dest = fb.buffer_mut();
            for x in 0..screen_w {
                dest[row_start + x] = color;
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────────
// TOUCH HANDLING (RETURNS THE ACTION IF THE TOUCH HITS THE CURRENT BUTTON)
pub fn handle_touch(x: i32, y: i32) -> core::option::Option<crate::gui::TouchAction> {
    critical_section::with(|_cs| unsafe {
        let opt = core::ptr::read_volatile(core::ptr::addr_of!(HIT_AREA));
        match opt {
            core::option::Option::Some(area) if crate::gui::hit_test(x, y, &area) => {
                core::option::Option::Some(area.action)
            }
            _ => core::option::Option::None,
        }
    })
}

// ───────────────────────────────────────────────────────────────────────
// SWIPE HANDLING (SCROLL THE LIST WHEN SPLIT IS OPEN)
pub fn handle_swipe(dir: crate::components::gt911::SwipeDirection) {
    if !is_split_open() {
        return;
    }
    let step = 70;
    critical_section::with(|cs| {
        let mut scroll = RSSI_SCROLL.borrow_ref_mut(cs);
        match dir {
            crate::components::gt911::SwipeDirection::Up => {
                scroll.target = (scroll.target + step).min(scroll.max_scroll);
            }
            crate::components::gt911::SwipeDirection::Down => {
                scroll.target = (scroll.target - step).max(0);
            }
            _ => {}
        }
    });
}

// ───────────────────────────────────────────────────────────────────────
// ASYNC FUNCTION TO TRIGGER SCAN AND SHOW RESULTS
pub async fn trigger_scan_and_show_results() {
    // START THE SCAN
    crate::base::wifi::scan().await;

    // WAIT UNTIL RESULTS APPEAR
    loop {
        let empty = {
            let guard = crate::base::wifi::SCAN_RESULTS.try_lock().unwrap();
            guard.is_empty()
        };
        if !empty {
            break;
        }
        embassy_time::Timer::after_millis(100).await;
    }

    // MARK SCAN LIST DIRTY SO IT GETS RENDERED INTO THE CACHE
    invalidate_scan_list();

    // OPEN THE SPLIT (STARTS THE ANIMATION)
    open_split();

    // FORCE A REDRAW
    crate::dirty!();
}

// ───────────────────────────────────────────────────────────────────────
// FORMATTING
fn format_rssi(rssi: i32) -> heapless::String<16> {
    let mut s = heapless::String::new();
    core::fmt::Write::write_fmt(&mut s, format_args!("{} dBm", rssi)).ok();
    s
}
