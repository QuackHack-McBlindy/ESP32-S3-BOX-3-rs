// BASE/ROUTES/API/SETTINGS/DISPLAY/IMAGE

// GET /API/SETTINGS/DISPLAY/IMAGE/{val}
pub async fn draw_image_handler(req: tinyapi::AsyncRequest) -> tinyapi::Response {
    let value = req.param("value").unwrap_or("?");

    let msg = alloc::format!("..");
    tinyapi::Response::text(&msg)
}
