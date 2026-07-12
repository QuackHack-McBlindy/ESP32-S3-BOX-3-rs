// BASE/ROUTES/API/SETTINGS/DISPLAY/STATE


// GET /API/SETTINGS/DISPLAY
pub async fn display_state_handler(req: tinyapi::AsyncRequest) -> tinyapi::Response {
    let value = req.param("value").unwrap_or("toggle");
    let desired = match value {
        "1" | "on" | "start" | "enable" | "enabled"   => true,
        "0" | "off" | "stop" | "disable" | "disabled"  => false,
        _ => !crate::load!(crate::state::DISPLAY_STATE), // toggle
    };

    crate::store!(crate::state::DISPLAY_STATE, desired);

    if desired {
        crate::DISPLAY_CMD.send(crate::DisplayCommand::Start).await;
        crate::store!(crate::state::DISPLAY_TOUCH_ACTIVITY, true);
    } else {
        crate::DISPLAY_CMD.send(crate::DisplayCommand::Stop).await;
    }

    let state = crate::load!(crate::state::DISPLAY_STATE);
    defmt::info!("Display state is now {}", if state { "ON" } else { "OFF" });
    tinyapi::Response::text(if state { "ON" } else { "OFF" })
}
