use tinyapi::{log, register_route, Request, Response};
use defmt::info;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use crate::BACKLIGHT_PERCENT;
use crate::media;
use crate::aht20::HUMIDITY;
use crate::aht20::TEMPERATURE;
use crate::presence::PRESENCE;
pub static POWER_STATE: AtomicBool = AtomicBool::new(true);
pub static DISPLAY_STATE: AtomicBool = AtomicBool::new(true);
pub static MIC_VOLUME: AtomicU8 = AtomicU8::new(72);
pub static SPEAKER_VOLUME: AtomicU8 = AtomicU8::new(58);
pub static MIC_MUTED: AtomicBool = AtomicBool::new(false);
pub static SPEAKER_MUTED: AtomicBool = AtomicBool::new(false);
use crate::media::{PLAYER, PLAYLIST, PlaybackState};
use alloc::vec;


fn api_list_handler(_req: Request<'_>) -> Response {
    let endpoints = vec![
        "/",
        "/api/settings/power/state/{value}",
        "/api/settings/display/state/{value}",
        // ...
    ];
    Response::text(&endpoints.join("\n"))
}

fn index_handler(_req: Request<'_>) -> Response {
    Response::html(include_str!("./../assets/index.html"))
}

pub fn brightness_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("?");
    info!("Setting brightness to {}", value);
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        BACKLIGHT_PERCENT.store(percent, Ordering::Relaxed);
    }
    let msg = format!("Brightness set to {}", value);
    Response::text(&msg)
}


fn power_state_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("toggle");
    match value {
        "on" => POWER_STATE.store(true, Ordering::Relaxed),
        "off" => POWER_STATE.store(false, Ordering::Relaxed),
        _ => {
            let new = !POWER_STATE.load(Ordering::Relaxed);
            POWER_STATE.store(new, Ordering::Relaxed);
        }
    }
    let state = POWER_STATE.load(Ordering::Relaxed);
    info!("Power state -> {}", if state { "ON" } else { "OFF" });
    Response::text(if state { "ON" } else { "OFF" })
}

fn display_state_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("toggle");
    match value {
        "on" => DISPLAY_STATE.store(true, Ordering::Relaxed),
        "off" => DISPLAY_STATE.store(false, Ordering::Relaxed),
        _ => {
            let new = !DISPLAY_STATE.load(Ordering::Relaxed);
            DISPLAY_STATE.store(new, Ordering::Relaxed);
        }
    }
    let state = DISPLAY_STATE.load(Ordering::Relaxed);
    info!("Display state -> {}", if state { "ON" } else { "OFF" });
    Response::text(if state { "ON" } else { "OFF" })
}

fn mic_volume_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("?");
    if let Ok(vol) = value.parse::<u8>() {
        let vol = vol.clamp(0, 100);
        MIC_VOLUME.store(vol, Ordering::Relaxed);
        info!("Mic volume set to {}%", vol);
    }
    Response::text(&format!("Mic volume {}", value))
}

fn mic_mute_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("toggle");
    match value {
        "1" | "on" | "mute" => MIC_MUTED.store(true, Ordering::Relaxed),
        "0" | "off" | "unmute" => MIC_MUTED.store(false, Ordering::Relaxed),
        _ => {
            let new = !MIC_MUTED.load(Ordering::Relaxed);
            MIC_MUTED.store(new, Ordering::Relaxed);
        }
    }
    let muted = MIC_MUTED.load(Ordering::Relaxed);
    if muted {
        MIC_VOLUME.store(0, Ordering::Relaxed);
    } else {
        MIC_VOLUME.store(72, Ordering::Relaxed);
    }
    info!("Mic muted: {}", muted);
    Response::text(if muted { "muted" } else { "unmuted" })
}

fn speaker_volume_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("?");
    if let Ok(vol) = value.parse::<u8>() {
        let vol = vol.clamp(0, 100);
        SPEAKER_VOLUME.store(vol, Ordering::Relaxed);
        info!("Speaker volume set to {}%", vol);
    }
    Response::text(&format!("Speaker volume {}", value))
}

fn speaker_mute_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("toggle");
    match value {
        "1" | "on" | "mute" => SPEAKER_MUTED.store(true, Ordering::Relaxed),
        "0" | "off" | "unmute" => SPEAKER_MUTED.store(false, Ordering::Relaxed),
        _ => {
            let new = !SPEAKER_MUTED.load(Ordering::Relaxed);
            SPEAKER_MUTED.store(new, Ordering::Relaxed);
        }
    }
    let muted = SPEAKER_MUTED.load(Ordering::Relaxed);
    if muted {
        SPEAKER_VOLUME.store(0, Ordering::Relaxed);
    } else {
        SPEAKER_VOLUME.store(58, Ordering::Relaxed);
    }
    info!("Speaker muted: {}", muted);
    Response::text(if muted { "muted" } else { "unmuted" })
}

fn detected_handler(req: Request<'_>) -> Response {
    let value = "70";
    info!("Setting brightness to {}", value);
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        BACKLIGHT_PERCENT.store(percent, Ordering::Relaxed);
    }
    Response::text("OK")
}

fn voice_win_handler(req: Request<'_>) -> Response {
    let value = "0";
    info!("Setting brightness to {}", value);
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        BACKLIGHT_PERCENT.store(percent, Ordering::Relaxed);
    }
    Response::text("OK")    
}

fn voice_fail_handler(req: Request<'_>) -> Response {
    let value = "0";
    info!("Setting brightness to {}", value);
    if let Ok(percent) = value.parse::<u8>() {
        let percent = percent.clamp(0, 80);
        BACKLIGHT_PERCENT.store(percent, Ordering::Relaxed);
    }
    Response::text("OK")
}

fn ota_handler(_req: Request<'_>) -> Response {
    info!("OTA update requested");
    Response::text("update started")
}

fn media_handler(req: Request<'_>) -> Response {
    let action = req.param("action").unwrap_or("none");
    info!("Media action: {}", action);
    let status = crate::media::handle_action(action);
    Response::text(status)
}

fn sensor_fetcher(req: Request<'_>) -> Response {
    let sensor_name = req.param("value").unwrap_or("unknown");
    info!("Sensor fetch requested: {}", sensor_name);

    let value = match sensor_name {
        "temp" | "temperature" => "23.6",
        "hum" | "humidity" => "48",
        "battery" | "battery_level" | "battery_percentage" => "78",
        "bbattery_voltage" | "voltage" => "3.84",
        "occupancy" | "motion" | "presence" => "Clear",
        "rssi" | "wifi_signal" | "wifi" => "-54",
        "ip" => "192.168.1.122",
        "uptime" => "3d 14h",
        "firmware" | "version" => "v2.1.0",
        _ => "unknown",
    };
    Response::text(value)
}

fn favicon_handler(_req: Request<'_>) -> Response {
    //Response::file(include_bytes!("./../assets/favicon.ico"));
    Response::not_found()    
}



fn js_handler(_req: Request<'_>) -> Response {
    Response::script(include_str!("./../assets/script.js"))
}

fn voice_state_handler(req: Request<'_>) -> Response {
    let value = req.param("value").unwrap_or("toggle");
    match value {
        "start" => {
            info!("Voice recording started");
        }
        "stop" => {
            info!("Voice recording stopped");
        }
        _ => {
            info!("Invalid voice state: {}", value);
            return Response::text("invalid state (use start/stop)");
        }
    }
    Response::text("ok")
}

pub async fn init_routes() {
    // Serve the web frontend
    register_route("/", index_handler).await;
    register_route("/favicon.ico", favicon_handler).await;
    register_route("/script.js", js_handler).await;
    // OTA
    register_route("/api/update", ota_handler).await;        
    // CONTROLLER ENDPOINTS
    register_route("/api/settings/power/state/{value}", power_state_handler).await;
    register_route("/api/settings/display/state/{value}", display_state_handler).await;
    register_route("/api/settings/display/brightness/{value}", brightness_handler).await;
    register_route("/api/settings/mic/volume/{value}", mic_volume_handler).await;
    register_route("/api/settings/mic/mute/{value}", mic_mute_handler).await;
    register_route("/api/settings/speaker/volume/{value}", speaker_volume_handler).await;
    register_route("/api/settings/speaker/mute/{value}", speaker_mute_handler).await;
    register_route("/api/settings/voice/state/{value}", voice_state_handler).await;

    // VOICE
    register_route("/api/voice/detected", detected_handler).await;
    register_route("/api/voice/executed", voice_win_handler).await;
    register_route("/api/voice/failed", voice_fail_handler).await;        

    register_route("/api/media/{action}", media_handler).await;
    // DATA ENDPOINTS
    // handles all sensor values currently on the ESP32-S3-BOX-3
    register_route("/api", api_list_handler).await;
    register_route("/api/sensor/{value}", sensor_fetcher).await;

    tinyapi::log!("API routes registered");
    log!("API routes registered!")
}
