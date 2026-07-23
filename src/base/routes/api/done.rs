// BASE/ROUTES/API/DIONE

pub async fn done_handler(req: tinyapi::AsyncRequest) -> tinyapi::Response {
    yo_esp::play_done().await;
    crate::store!(crate::state::DISPLAY_BRIGHTNESS, 25);
    tinyapi::Response::text("Dong.")
}
