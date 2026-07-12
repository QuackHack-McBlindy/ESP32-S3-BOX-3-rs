// COMPONENTS/DISPLAY


pub const WIDTH: u16 = 320;
pub const HEIGHT: u16 = 240;

// ───────────────────────────────────────────────────────────────────────
// FLASH DISPLAY
pub fn flash(times: u32) {
    let original = crate::load!(crate::state::DISPLAY_BRIGHTNESS);
    for _ in 0..times {
        crate::store!(crate::state::DISPLAY_BRIGHTNESS, 80);
        crate::wait_ms!(200);
        crate::store!(crate::state::DISPLAY_BRIGHTNESS, 0);
        crate::wait_ms!(200);
    }
    crate::store!(crate::state::DISPLAY_BRIGHTNESS, original);
}


// ───────────────────────────────────────────────────────────────────────
// WAKEUP DISPLAY (BLOCKING)
pub fn display_on() {
    if crate::DISPLAY_CMD.try_send(crate::DisplayCommand::Start).is_err() {
        defmt::warn!("Display command channel full, command dropped");
    }
}

// ───────────────────────────────────────────────────────────────────────
// STOP DISPLAY (BLOCKING)
pub fn display_off() {
    if crate::DISPLAY_CMD.try_send(crate::DisplayCommand::Stop).is_err() {
        defmt::warn!("Display command channel full, command dropped");
    }
}
