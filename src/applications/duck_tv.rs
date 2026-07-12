// APPLICATIONS/DUCK_TV

// DUCK-TV IS MY PERSONAL TVOS/ANDROID/BROWSER LOCAL MEDIA STREAMING APP
// THE EMBEDDED VERSIONS LET'S ME BROWSE MUSIC/MOVIES/TV SHOWS STORED ON A NAS
// AND CAST CHOSEN MEDIA TO THE TV (TLS / SOME DNS ROUTING REQUIRED)

// ───────────────────────────────────────────────────────────────────────
// DESCRIBE THIS APPLICATION
pub const APP_DESCRIPTOR: crate::applications::AppDescriptor = crate::applications::AppDescriptor {
    name: "duck-tv",
    description: "duck-tv watch controller",
    launch: open_app,
    icon: crate::base::assets::DUCK_TV_PNG,
};

pub fn open_app() { crate::store!(crate::gui::pages::CURRENT_PAGE, crate::gui::pages::Page::DuckTv as u8); }

// ───────────────────────────────────────────────────────────────────────
// CONSTANTS

pub const MAX_BROWSE_ITEMS: usize = 96;

pub static BROWSE_ITEMS: critical_section::Mutex<
    core::cell::RefCell<heapless::Vec<heapless::String<128>, MAX_BROWSE_ITEMS>>,
> = critical_section::Mutex::new(core::cell::RefCell::new(heapless::Vec::new()));

pub static BROWSE_PATH: critical_section::Mutex<
    core::cell::RefCell<heapless::String<128>>,
> = critical_section::Mutex::new(core::cell::RefCell::new(heapless::String::new()));

pub static BROWSE_CATEGORY: critical_section::Mutex<
    core::cell::RefCell<heapless::String<16>>,
> = critical_section::Mutex::new(core::cell::RefCell::new(heapless::String::new()));

// ───────────────────────────────────────────────────────────────────────
// TYPES & GLOBAL STATE
#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum TvCommand {
    Play,
    Pause,
    Next,
    Prev,
    Stop,
    Clear,
    Heart,
}

pub static TV_CMD: embassy_sync::channel::Channel<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    TvCommand,
    4,
> = embassy_sync::channel::Channel::new();


#[derive(Clone, Copy, Debug, PartialEq, defmt::Format)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}


// ───────────────────────────────────────────────────────────────────────
// HELPER TO CALL ZIGDUCK-API
async fn send_home_command(cmd: crate::applications::zigduck::HomeCommand) {
    crate::applications::zigduck::HOME_CMD.send(cmd).await;
}

// HELPER TO START MEDIA USING NATURAL LANGUAGE (Swedish)
pub fn send_media_command(category: &str, name: &str) {
    let cmd = match category {
        "TV"     => alloc::format!("spela upp serien {}", name),
        "Music"  => alloc::format!("spela upp artisten {}", name),
        "Movies" => alloc::format!("spela upp filmen {}", name),
        _        => alloc::format!("spela upp {}", name),
    };
    crate::yo!(cmd.as_str());
    crate::store!(crate::state::TV_IS_ON, true);
    crate::store!(crate::state::TV_IS_PLAYING, true);
}

// HELPER THAT RETURNS THE CURRENT BROWSED MEDIA TYPE
pub fn browse_category() -> Option<heapless::String<16>> {
    critical_section::with(|cs| {
        let cat = BROWSE_CATEGORY
            .borrow_ref(cs)
            .clone();
        if cat.is_empty() { None } else { Some(cat) }
    })
}

// RETURNS THE NAME OF THE BROWSE ITEM AT THE GIVEN INDEX
pub fn browse_item_name(index: usize) -> Option<heapless::String<128>> {
    critical_section::with(|cs| {
        BROWSE_ITEMS
            .borrow_ref(cs)
            .get(index)
            .cloned()
    })
}


// ───────────────────────────────────────────────────────────────────────
// INCREASE VOLUME (PUSH UPPER RIGHT BUTTON WHILE IN THE APP)
pub async fn volume_up() {
    let current = crate::load!(crate::state::TV_VOLUME);
    let new = (current + 5).min(90); // +5 STEP SIZE
    defmt::info!("📺 Volume: {}", new);
    crate::store!(crate::state::TV_VOLUME, new);
    send_home_command(crate::applications::zigduck::HomeCommand::MediaVolumeUp(Some(crate::get_string!(crate::state::TV_IP)))).await;
}

// DECREASE VOLUME (PUSH LOWER RIGHT BUTTON WHILE IN THE APP)
pub async fn volume_down() {
    let current = crate::load!(crate::state::TV_VOLUME);
    let new = current.saturating_sub(5); // -5 STEP SIZE
    defmt::info!("📺 Volume: {}", new);
    send_home_command(crate::applications::zigduck::HomeCommand::MediaVolumeDown(Some(crate::get_string!(crate::state::TV_IP)))).await;
}

// POWER ON/OFF
pub async fn power() {
    let is_on = crate::load!(crate::state::TV_IS_ON);
    if is_on {
        send_home_command(crate::applications::zigduck::HomeCommand::MediaPowerOff(Some(crate::get_string!(crate::state::TV_IP)))).await;
        crate::store!(crate::state::TV_IS_ON, false);
    } else {
        send_home_command(crate::applications::zigduck::HomeCommand::MediaPowerOn(Some(crate::get_string!(crate::state::TV_IP)))).await;
        crate::store!(crate::state::TV_IS_ON, true);
    }
}

// PLAY
pub async fn play() -> Result<(), &'static str> {
    let _ = TV_CMD.send(TvCommand::Play).await;
    Ok(())
}

// PAUSE
pub async fn pause() -> Result<(), &'static str> {
    let _ = TV_CMD.send(TvCommand::Pause).await;
    Ok(())
}

// PLAY/PAUSE
pub async fn play_pause() -> Result<(), &'static str> {
    let is_playing = crate::load!(crate::state::TV_IS_PLAYING);
    if is_playing { 
        let _ = TV_CMD.send(TvCommand::Pause).await;
    } else { let _ = TV_CMD.send(TvCommand::Play).await; }
    Ok(())
}

// NEXT TRACK
pub async fn next() {
    let _ = TV_CMD.send(TvCommand::Next).await;
}

// PREVIOUS TRACK
pub async fn prev() {
    let _ = TV_CMD.send(TvCommand::Prev).await;
}

// BROWSE MEDIA: /TV
pub async fn tv() {
    crate::applications::zigduck::HOME_CMD.send(crate::applications::zigduck::HomeCommand::BrowseTv(heapless::String::<64>::new())).await;
}

// BROWSE MEDIA: /MOVIES
pub async fn movies() {
    crate::applications::zigduck::HOME_CMD.send(crate::applications::zigduck::HomeCommand::BrowseMovies(heapless::String::<64>::new())).await;
}

// BROWSE MEDIA: /MUSIC
pub async fn music() {
    crate::applications::zigduck::HOME_CMD.send(crate::applications::zigduck::HomeCommand::BrowseMusic(heapless::String::<64>::new())).await;
}

// ───────────────────────────────────────────────────────────────────────
// TASK
#[embassy_executor::task]
pub async fn tv_task(stack: &'static embassy_net::Stack<'static>) {
    let mut state = PlaybackState::Stopped;

    loop {
        match state {
        
            // ───────────────────────────────────────────────────────────────────────
            // STATE: STOPPED / PAUSED
            PlaybackState::Stopped | PlaybackState::Paused => {
                // IDLE - WAIT FOR A COMMAND
                let cmd = TV_CMD.receive().await;
                match cmd {
                
                    // ───────────────────────────────────────────────────────────────────────
                    // PLAY COMMAND (WHILE STOPPED/PAUSED)
                    TvCommand::Play => {
                        send_home_command(crate::applications::zigduck::HomeCommand::MediaPlay(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        state = PlaybackState::Playing;
                        crate::store!(crate::state::TV_IS_PLAYING, true);
                    } 
                  
                    // ───────────────────────────────────────────────────────────────────────
                    // PREVIOUS COMMAND (WHILE STOPPED/PAUSED)
                    TvCommand::Prev => {
                        send_home_command(crate::applications::zigduck::HomeCommand::MediaPrev(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        state = PlaybackState::Playing;
                    }

                    // ───────────────────────────────────────────────────────────────────────
                    // NEXT COMMAND (WHILE STOPPED/PAUSED)
                    TvCommand::Next => {
                        send_home_command(crate::applications::zigduck::HomeCommand::MediaNext(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        state = PlaybackState::Playing;
                    }
                    
                    // ───────────────────────────────────────────────────────────────────────
                    // PAUSE COMMAND (WHILE STOPPED/PAUSED)
                    TvCommand::Pause => {
                        // ALREADY PAUSED/STOPPED - DO NOTHING
                    }
                    
                    // ───────────────────────────────────────────────────────────────────────
                    // STOP COMMAND (WHILE STOPPED/PAUSED)
                    TvCommand::Stop => {
                        state = PlaybackState::Stopped;
                        crate::store!(crate::state::TV_IS_PLAYING, false);
                    }

                    // ───────────────────────────────────────────────────────────────────────
                    // UNKNOWN COMMAND (WHILE STOPPED/PAUSED)
                    _ => { defmt::info!("Command {:?} ignored in {:?} state", cmd, state); }
                }
            }
 
            // ───────────────────────────────────────────────────────────────────────
            // STATE: PLAYING
            PlaybackState::Playing => {
            
                // CHECK FOR NEW COMMANDS WITHOUT BLOCKING
                if let Ok(cmd) = TV_CMD.try_receive() {
                    match cmd {

                        // ───────────────────────────────────────────────────────────────────────
                        // PAUSE COMMAND (WHILE PLAYING)
                        TvCommand::Pause => {
                            state = PlaybackState::Paused;
                            send_home_command(crate::applications::zigduck::HomeCommand::MediaPause(Some(crate::get_string!(crate::state::TV_IP)))).await;
                            crate::store!(crate::state::TV_IS_PLAYING, false);
                            continue;
                        }
                        
                        // ───────────────────────────────────────────────────────────────────────
                        // STOP COMMAND (WHILE PLAYING)
                        TvCommand::Stop => {
                            state = PlaybackState::Stopped;
                            crate::store!(crate::state::TV_IS_PLAYING, false);
                            send_home_command(crate::applications::zigduck::HomeCommand::MediaPause(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        }
                        
                        // ───────────────────────────────────────────────────────────────────────
                        // PREVIOUS COMMAND (WHILE PLAYING)
                        TvCommand::Prev => {
                            send_home_command(crate::applications::zigduck::HomeCommand::MediaPrev(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        }
     
                        // ───────────────────────────────────────────────────────────────────────
                        // NEXT COMMAND (WHILE PLAYING)     
                        TvCommand::Next => {
                            send_home_command(crate::applications::zigduck::HomeCommand::MediaNext(Some(crate::get_string!(crate::state::TV_IP)))).await;
                        }

                        
                        // ───────────────────────────────────────────────────────────────────────
                        // PLAY COMMAND (WHILE PLAYING)
                        TvCommand::Play => {
                            // ALREADY PLAYING - IGNORE
                        }
                                             
                        // ───────────────────────────────────────────────────────────────────────
                        // CLEAR COMMAND (WHILE PLAYING)
                        TvCommand::Clear => {
                            // CLEAR THE PLAYLIST
                            defmt::debug!("Received Clear command");
                        }
                        
                        // ───────────────────────────────────────────────────────────────────────
                        // HEART COMMAND (WHILE PLAYING)
                        TvCommand::Heart => {
                            defmt::debug!("Received Heart command");
                        }
                    }
                }
            }
        }
    }
}



// ───────────────────────────────────────────────────────────────────────

