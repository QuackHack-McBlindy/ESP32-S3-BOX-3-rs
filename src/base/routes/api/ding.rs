// BASE/ROUTES/API/DING

pub async fn ding_handler(req: tinyapi::AsyncRequest) -> tinyapi::Response {
    yo_esp::play_ding().await;
    crate::store!(crate::state::DISPLAY_BRIGHTNESS, 70);
    tinyapi::Response::text("Dong.")
}
