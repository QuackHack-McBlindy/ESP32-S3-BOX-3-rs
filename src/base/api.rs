// BASE/API
// CONFIGURES `GET` ENDPOINTS VIA `tinyapi`
// FOR CONTROLLING/CONFIGURING THE ESP32 EXTERNALLY & VIA VOICE COMMANDS
// ++ SERVE WEBSERVER AT `http://0.0.0.0:80`
// EXAMPLE USAGE: (SET DISPLAY BRIGHTNESS TO `70%` USING `curl`) 
// `curl 192.168.1.11/api/settings/display/brightness/70`


// ───────────────────────────────────────────────────────────────────────
// FUNCTION TO INIT ENDPOINTS
pub async fn init_routes() {
    // SERVE THE WEB FRONTEND
    //tinyapi::register_route("/", crate::base::routes::index::index_handler).await;   
   // tinyapi::register_route("/favicon.ico", crate::base::routes::index::favicon_handler).await;    

   // tinyapi::register_route("/www/{file}", crate::base::routes::index::serve_file_handler).await;

    // ───────────────────────────────────────────────────────────────────────
    // /API (GET)
    // LIST AVAILABLE ENDPOINTS
    tinyapi::register_route("/api", crate::base::routes::api::list::handle).await;

    // ───────────────────────────────────────────────────────────────────────
    // /API/DING (GET)

    // DING PLAY SOUND A START DISPLAY
    tinyapi::register_async_route("/api/ding", crate::base::routes::api::ding::ding_handler).await;

    // /API/DONE (GET)
    tinyapi::register_async_route("/api/done", crate::base::routes::api::done::done_handler).await;


    // ───────────────────────────────────────────────────────────────────────
    // /API/PROCESS (GET)

    // PROCESS A SENTENCE, EXTRACT PARAMETRS & EXECUTE
    tinyapi::register_async_route("/api/process/{value}", crate::base::routes::api::process::sentence_handler).await;


    // ───────────────────────────────────────────────────────────────────────
    // /API/WEATHER (GET)

    // UPDATE
    tinyapi::register_async_route("/api/weather/update", crate::base::routes::api::weather::update::weather_handler).await;


    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/API (GET)

    // OFF
    tinyapi::register_async_route("/api/settings/api/off", crate::base::routes::api::settings::api::off::disable_api).await;  



    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/CPU (GET)
    
    // SET CPU FREQUENCY (80, 160, 240)
    tinyapi::register_async_route("/api/settings/cpu/{value}", crate::base::routes::api::settings::cpu::set::cpu_handler).await;
    

    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/MIC (GET)
    
    // VOLUME    
    tinyapi::register_route("/api/settings/mic/volume/{value}", crate::base::routes::api::settings::mic::volume::mic_volume_handler).await;
    
    // MUTE
    tinyapi::register_route("/api/settings/mic/mute/{value}", crate::base::routes::api::settings::mic::mute::mic_mute_handler).await;


    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/SPEAKER (GET)

    // ON/OFF
    tinyapi::register_async_route("/api/settings/speaker/{value}", crate::base::routes::api::settings::speaker::toggle::toggle_handler).await;
    
    // VOLUME (0-100)  
    tinyapi::register_route("/api/settings/speaker/volume/{value}", crate::base::routes::api::settings::speaker::volume::speaker_volume_handler).await;

    // MUTE (on/off)  
    tinyapi::register_route("/api/settings/speaker/mute/{value}", crate::base::routes::api::settings::speaker::mute::speaker_mute_handler).await;  

    // AMP (on/off/toggle)  
    tinyapi::register_route("/api/settings/speaker/amp/{value}", crate::base::routes::api::settings::speaker::amp::amp_handler).await;  


    // STREAM (on/off)
    tinyapi::register_async_route("/api/settings/speaker/stream/{value}", crate::base::routes::api::settings::speaker::stream::stream_handler).await;  

    // DING (PLAYS SOUND)
    tinyapi::register_async_route("/api/settings/speaker/play/ding", crate::base::routes::api::settings::speaker::ding::ding_handler).await;  


    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/VOICE (GET)

    // ON/OFF/TOGGLE (THE ENTIRE PIPELINE)
    tinyapi::register_async_route("/api/settings/voice/{value}", crate::base::routes::api::settings::voice::state::voice_handler).await;
        
    // WAKEWORD (on/off) 
    tinyapi::register_async_route("/api/settings/voice/wakeword/{value}", crate::base::routes::api::settings::voice::wakeword::wake_word_handler).await;
    

    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/DISPLAY (GET)
    
    // BRIGHTNESS
    tinyapi::register_route("/api/settings/display/brightness/{value}", crate::base::routes::api::settings::display::brightness::brightness_handler).await;    

    // STATE (on/off/toggle)
    tinyapi::register_async_route("/api/settings/display/state/{value}", crate::base::routes::api::settings::display::state::display_state_handler).await;

    // PAGE
    tinyapi::register_route("/api/settings/display/page/{value}", crate::base::routes::api::settings::display::page::page_handler).await;



    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/VPN (GET)
    
    // STATE 
    tinyapi::register_async_route("/api/settings/vpn/{val}", crate::base::routes::api::settings::vpn::state::vpn_handler).await;

    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/WIFI (GET)
    
    // OFF 
    tinyapi::register_route("/api/settings/wifi/off", crate::base::routes::api::settings::wifi::off::disable_wifi).await;

    // SCAN
    tinyapi::register_async_route("/api/settings/wifi/scan", crate::base::routes::api::settings::wifi::scan::scan_handler).await;


    // ───────────────────────────────────────────────────────────────────────
    // /API/SETTINGS/BLUETOOTH (GET)

    // ... (TODO)    

    // ───────────────────────────────────────────────────────────────────────


}
