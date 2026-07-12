// APPLICATIONS/ZIGDUCK
// THIS APPLICATION ONLY TALKS TO THE [zigduck-api](https://github.com/QuackHack-McBlindy/zigduck2mqttnix)


// ───────────────────────────────────────────────────────────────────────
// DESCRIBE THIS APPLICATION
pub const APP_DESCRIPTOR: crate::applications::AppDescriptor = crate::applications::AppDescriptor {
    name: "zigduck",
    description: "Just some place to store home automation logic.",
    launch: open_app,
    icon: crate::base::assets::HOUSE_PNG,
};

pub fn open_app() { crate::store!(crate::gui::pages::CURRENT_PAGE, crate::gui::pages::Page::Zigduck as u8); }

// ───────────────────────────────────────────────────────────────────────
// HELPERS

fn url_encode<const N: usize>(input: &str, buf: &mut heapless::String<N>) -> Result<(), ()> {
    // ROUGH CAPACITY CHECK (WORST CASE SCENARIO 3x EXPANSION)
    if input.len() * 3 > buf.capacity() - buf.len() {
        return Err(());
    }
    for byte in input.bytes() {
        match byte {
            // KEEP THEM AS IS        
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => {
                buf.push(byte as char)?;
            }
            // SPACE = `+`            
            b' ' => {
                buf.push('+')?;
            }
            // EVERYTHING ELSE = `%XX`
            _ => {
                use core::fmt::Write;
                write!(buf, "%{:02X}", byte).map_err(|_| ())?;
            }
        }
    }
    Ok(())
}

// PARSE JSON FROM BROWSED MEDIA
fn parse_directories(json: &str) -> heapless::Vec<heapless::String<128>, { crate::applications::duck_tv::MAX_BROWSE_ITEMS }> {
    let mut dirs = heapless::Vec::new();
    // QUICK & DIRTY - FIND "directories": [...] AND SPLIT BY COMMAS
    if let Some(start) = json.find("\"directories\":[") {
        let start = start + 15;
        if let Some(end) = json[start..].find(']') {
            let inner = &json[start..start+end];
            for part in inner.split(',') {
                let cleaned = part.trim().trim_matches('"').trim();
                if !cleaned.is_empty() {
                    if let Ok(s) = heapless::String::try_from(cleaned) {
                        let _ = dirs.push(s);
                    }
                }
            }
        }
    }
    dirs
}

// ───────────────────────────────────────────────────────────────────────

#[derive(Clone, defmt::Format)]
pub enum HomeCommand {
    ToggleLights,
    Scene(heapless::String<32>),
    DeviceState(heapless::String<32>, bool),
    Nlp(heapless::String<64>),     
    MediaPowerOn(Option<heapless::String<16>>),
    MediaPowerOff(Option<heapless::String<16>>),
    MediaNext(Option<heapless::String<16>>),
    MediaPrev(Option<heapless::String<16>>),
    MediaPlay(Option<heapless::String<16>>),    
    MediaPause(Option<heapless::String<16>>),
    MediaVolumeUp(Option<heapless::String<16>>),
    MediaVolumeDown(Option<heapless::String<16>>),
    BrowseTv(heapless::String<64>),
    BrowseMovies(heapless::String<64>),
    BrowseMusic(heapless::String<64>),
}

pub static HOME_CMD: embassy_sync::channel::Channel<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    HomeCommand,
    4,
> = embassy_sync::channel::Channel::new();


// ───────────────────────────────────────────────────────────────────────
// EMBASSY TASK
#[embassy_executor::task]
pub async fn smart_home_task(stack: &'static embassy_net::Stack<'static>) {
    loop {
        let base_url = crate::state::ZIGDUCK_BASE_URL.unwrap_or("http://192.168.1.211:13335");
        let cmd = HOME_CMD.receive().await;
        match cmd {
            // NATURAL LANGUAGE PROCESSING
            HomeCommand::Nlp(command) => {
                let mut encoded = heapless::String::<256>::new();
                if url_encode(&command, &mut encoded).is_err() {
                    defmt::warn!("command too long to encode");
                    continue;
                }
                let url = alloc::format!(
                    "{}/do?cmd={}",
                    base_url,
                    encoded
                );
                let mut buf = [0u8; 4096];

                match tinyapi::http_get_auth(
                    stack,
                    &url,
                    crate::state::ZIGDUCK_API_PASSWORD,
                    &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::info!("NLP command '{}' succeeded", command); }
                    Ok(resp) => { defmt::warn!("NLP command '{}' failed with status {}", command, resp.status); }
                    Err(_) => { defmt::error!("Network error sending NLP command '{}'", command); }
                }
            }

            // CONTROL INDIVIDUAL DEVICE STATES (ON/OFF)
            HomeCommand::DeviceState(device, state_on) => {
                let action = if state_on { "on" } else { "off" };
                let url = alloc::format!(
                    "{}/device/{}/state/{}",
                    base_url,
                    device, action
                );
                let mut buf = [0u8; 512];

                match tinyapi::http_get_auth(
                    stack,
                    &url,
                    crate::state::ZIGDUCK_API_PASSWORD,
                    &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::info!("Device '{}' turned {}", device, action); }
                    Ok(resp) => { defmt::warn!("Device '{}' control failed (status {})", device, resp.status); }
                    Err(_) => { defmt::error!("Network error controlling device '{}'", device); }
                }               
            }
    
            // TOGGLE ALL LIGHTS ON/OFF
            HomeCommand::ToggleLights => {
                let scene = if crate::load!(crate::state::LIGHTS_STATE) {
                    "max"
                } else { "dark" };
                let url = alloc::format!(
                    "{}/scene/{}",
                    base_url,
                    scene
                );
                let mut buf = [0u8; 512];

                match tinyapi::http_get_auth(
                    stack,
                    &url,
                    crate::state::ZIGDUCK_API_PASSWORD,
                    &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::info!("Scene '{}' activated", scene); }
                    Ok(resp) => { defmt::info!("Scene '{}' failed with status {}", scene, resp.status); }
                    Err(_) => { defmt::info!("Network error activating scene '{}'", scene); }
                }
            }
            
            // SET A SCENE BY NAME
            HomeCommand::Scene(name) => {
                let url = alloc::format!(
                    "{}/scene/{}",
                    base_url,
                    name
                );
                let mut buf = [0u8; 512];

                match tinyapi::http_get_auth(
                    stack,
                    &url,
                    crate::state::ZIGDUCK_API_PASSWORD,
                    &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::info!("Scene '{}' activated", name); }
                    Ok(resp) => { defmt::info!("Scene '{}' failed with status {}", name, resp.status); }
                    Err(_) => { defmt::info!("Network error activating scene '{}'", name); }
                }
            }    

            // TV: POWER ON
            HomeCommand::MediaPowerOn(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!("{}/media/power/on?device={}", base_url, device_ip);
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(stack, &url, crate::state::ZIGDUCK_API_PASSWORD, &mut buf).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: power on"); }
                    Ok(resp) => { defmt::warn!("Media power on failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media power on"); }
                }
            }

            // TV: POWER OFF
            HomeCommand::MediaPowerOff(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!("{}/media/power/off?device={}", base_url, device_ip);
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(stack, &url, crate::state::ZIGDUCK_API_PASSWORD, &mut buf).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: power off"); }
                    Ok(resp) => { defmt::warn!("Media power off failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media power off"); }
                }
            }
                        
            // TV: NEXT TRACK
            HomeCommand::MediaNext(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/next?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: next track"); }
                    Ok(resp) => { defmt::warn!("Media next failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media next"); }
                }
            }

            // TV: PREVIOUS TRACK
            HomeCommand::MediaPrev(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/previous?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: previous track"); }
                    Ok(resp) => { defmt::warn!("Media previous failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media previous"); }
                }
            }

            // TV: PAUSE
            HomeCommand::MediaPause(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/play?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: play/pause toggled"); }
                    Ok(resp) => { defmt::warn!("Media play/pause failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media play/pause"); }
                }
            }
            
            // TV: PLAY
            HomeCommand::MediaPlay(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/play?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: play/pause toggled"); }
                    Ok(resp) => { defmt::warn!("Media play/pause failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media play/pause"); }
                }
            }

            // TV: INCREASE VOLUME
            HomeCommand::MediaVolumeUp(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/volume/up?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: volume up"); }
                    Ok(resp) => { defmt::warn!("Media volume up failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media volume up"); }
                }
            }

            // TV: DECREASE VOLUME
            HomeCommand::MediaVolumeDown(device) => {
                let device_ip = crate::get_string!(crate::state::TV_IP);
                let url = alloc::format!(
                    "{}/media/volume/down?device={}",
                    base_url,
                    device_ip
                );
                let mut buf = [0u8; 256];
                match tinyapi::http_get_auth(
                    stack, &url,
                    crate::state::ZIGDUCK_API_PASSWORD, &mut buf,
                ).await {
                    Ok(resp) if resp.status == 200 => { defmt::debug!("API call: volume down"); }
                    Ok(resp) => { defmt::warn!("Media volume down failed with status {}", resp.status); }
                    Err(_) => { defmt::error!("Network error: media volume down"); }
                }
            }
 
            // BROWSE MEDIA: /TV
            HomeCommand::BrowseTv(path) => {
                let full_path = if path.is_empty() {
                    heapless::String::<128>::try_from("TV").unwrap()
                } else {
                    let mut s = heapless::String::<128>::try_from("TV/").unwrap();
                    s.push_str(&path).ok();
                    s
                };
                let mut encoded = heapless::String::<256>::new();
                url_encode(&full_path, &mut encoded);
                let url = alloc::format!("{}/browse?path={}", base_url, encoded);
                let mut buf = [0u8; 4096];
            
                match tinyapi::http_get_auth(stack, &url, crate::state::ZIGDUCK_API_PASSWORD, &mut buf).await {
                    Ok(resp) if resp.status == 200 => {
                        let body = core::str::from_utf8(resp.body).unwrap_or("");
                        defmt::debug!("TV browse: {}", body);
                        let body_str = core::str::from_utf8(resp.body).unwrap_or("");
                        let items = parse_directories(body_str);
                        critical_section::with(|cs| {
                            *crate::applications::duck_tv::BROWSE_ITEMS.borrow_ref_mut(cs) = items;
                            *crate::applications::duck_tv::BROWSE_PATH.borrow_ref_mut(cs) = full_path.clone();
                            *crate::applications::duck_tv::BROWSE_CATEGORY.borrow_ref_mut(cs) =
                                heapless::String::<16>::try_from("TV").unwrap();
                        });
                        crate::gui::duck_tv::invalidate_playlist();
                    }
                    Ok(resp) => defmt::warn!("Browse TV failed with status {}", resp.status),
                    Err(_) => defmt::error!("Network error browsing TV"),
                }
            }
            
            // BROWSE MEDIA: /Movies
            HomeCommand::BrowseMovies(path) => {
                let full_path = if path.is_empty() {
                    heapless::String::<128>::try_from("Movies").unwrap()
                } else {
                    let mut s = heapless::String::<128>::try_from("Movies/").unwrap();
                    s.push_str(&path).ok();
                    s
                };
                let mut encoded = heapless::String::<256>::new();
                url_encode(&full_path, &mut encoded);
                let url = alloc::format!("{}/browse?path={}", base_url, encoded);
                let mut buf = [0u8; 4096];
            
                match tinyapi::http_get_auth(stack, &url, crate::state::ZIGDUCK_API_PASSWORD, &mut buf).await {
                    Ok(resp) if resp.status == 200 => {
                        let body = core::str::from_utf8(resp.body).unwrap_or("");
                        defmt::debug!("Movies browse: {}", body);
                        let body_str = core::str::from_utf8(resp.body).unwrap_or("");
                        let items = parse_directories(body_str);
                        critical_section::with(|cs| {
                            *crate::applications::duck_tv::BROWSE_ITEMS.borrow_ref_mut(cs) = items;
                            *crate::applications::duck_tv::BROWSE_PATH.borrow_ref_mut(cs) = full_path.clone();
                            *crate::applications::duck_tv::BROWSE_CATEGORY.borrow_ref_mut(cs) =
                                heapless::String::<16>::try_from("Movies").unwrap();
                        });
                        crate::gui::duck_tv::invalidate_playlist();
                    }
                    Ok(resp) => defmt::warn!("Browse Movies failed with status {}", resp.status),
                    Err(_) => defmt::error!("Network error browsing Movies"),
                }
            }
            
            // BROWSE MEDIA: /Music
            HomeCommand::BrowseMusic(path) => {
                let full_path = if path.is_empty() {
                    heapless::String::<128>::try_from("Music").unwrap()
                } else {
                    let mut s = heapless::String::<128>::try_from("Music/").unwrap();
                    s.push_str(&path).ok();
                    s
                };
                let mut encoded = heapless::String::<256>::new();
                url_encode(&full_path, &mut encoded);
                let url = alloc::format!("{}/browse?path={}", base_url, encoded);
                let mut buf = [0u8; 8192];
            
                match tinyapi::http_get_auth(stack, &url, crate::state::ZIGDUCK_API_PASSWORD, &mut buf).await {
                    Ok(resp) if resp.status == 200 => {
                        let body = core::str::from_utf8(resp.body).unwrap_or("");
                        defmt::debug!("Music browse: {}", body);
                        let body_str = core::str::from_utf8(resp.body).unwrap_or("");
                        let items = parse_directories(body_str);
                        critical_section::with(|cs| {
                            *crate::applications::duck_tv::BROWSE_ITEMS.borrow_ref_mut(cs) = items;
                            *crate::applications::duck_tv::BROWSE_PATH.borrow_ref_mut(cs) = full_path.clone();
                            *crate::applications::duck_tv::BROWSE_CATEGORY.borrow_ref_mut(cs) =
                                heapless::String::<16>::try_from("Music").unwrap();
                        });
                        crate::gui::duck_tv::invalidate_playlist();
                    }
                    Ok(resp) => defmt::warn!("Browse Music failed with status {}", resp.status),
                    Err(_) => defmt::error!("Network error browsing Music"),
                }
            }         
            // MORE ... ?
            
        }        
    }
}
