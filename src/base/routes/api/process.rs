// BASE/ROUTES/API/PROCESS

pub async fn sentence_handler(req: tinyapi::AsyncRequest) -> tinyapi::Response {
    let value = req.param("value").unwrap_or("");
    crate::yo!(value);
    tinyapi::Response::text("Processing...")
}
