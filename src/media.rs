use core::cell::RefCell;
use critical_section::Mutex;
use defmt::{info, warn, error};
use crate::api::*;
use core::sync::atomic::Ordering;
use crate::{I2C_BUS, ES7210};



#[derive(Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone)]
pub struct Track {
    pub id: u32,
    pub title: &'static str,
    pub file_path: &'static str,  
}

pub const PLAYLIST: &[Track] = &[
    Track { id: 1, title: "Song One", file_path: "/music/one.mp3" },
    Track { id: 2, title: "Song Two", file_path: "/music/two.mp3" },
    Track { id: 3, title: "Song Three", file_path: "/music/three.mp3" },
];

struct PlayerInner {
    pub state: PlaybackState,
    pub current_track_index: usize,
}


pub static PLAYER: Mutex<RefCell<PlayerInner>> = Mutex::new(RefCell::new(PlayerInner {
    state: PlaybackState::Stopped,
    current_track_index: 0,
}));



fn audio_hardware_play(file_path: &str) -> Result<(), &'static str> {
    info!("Playing file: {}", file_path);
    Ok(())
}

fn audio_hardware_stop() {
    info!("Stopping audio playback");
}

fn audio_hardware_pause() {
    info!("Pausing audio playback");
}

fn audio_hardware_resume() {
    info!("Resuming audio playback");
}


pub fn handle_action(action: &str) -> &'static str {
    match action {
        "play" => {
            let _ = play();
            "Playing"
        }
        "pause" => {
            pause();
            "Paused"
        }
        "next" => {
            next();
            "Next track"
        }
        "prev" => {
            prev();
            "Previous track"
        }
        "stop" => {
            stop();
            "Stopped"
        }
        "status" => get_status_text(),
        "volume_up" => {
            volume_up();
            "Volume up"
        }
        "volume_down" => {
            volume_down();
            "Volume down"
        }
        _ => {
            warn!("Unknown media action: {}", action);
            "Unknown action"
        }
    }
}


pub fn get_status_text() -> &'static str {
    critical_section::with(|cs| {
        let player = PLAYER.borrow_ref(cs);
        let current_track = &PLAYLIST[player.current_track_index];
        let vol = SPEAKER_VOLUME.load(core::sync::atomic::Ordering::Relaxed);
        let muted = SPEAKER_MUTED.load(core::sync::atomic::Ordering::Relaxed);
    })
}


fn play() -> Result<(), &'static str> {
    critical_section::with(|cs| {
        let mut player = PLAYER.borrow_ref_mut(cs);
        let track = &PLAYLIST[player.current_track_index];

        // Stop
        audio_hardware_stop();

        // Start
        if let Err(e) = audio_hardware_play(track.file_path) {
            error!("Failed to play {}: {}", track.file_path, e);
            player.state = PlaybackState::Stopped;
            return Err(e);
        }

        player.state = PlaybackState::Playing;
        info!("Now playing: {}", track.title);
        Ok(())
    })
}

fn pause() {
    critical_section::with(|cs| {
        let mut player = PLAYER.borrow_ref_mut(cs);
        match player.state {
            PlaybackState::Playing => {
                audio_hardware_pause();
                player.state = PlaybackState::Paused;
                info!("Playback paused");
            }
            PlaybackState::Paused => {
                audio_hardware_resume();
                player.state = PlaybackState::Playing;
                info!("Playback resumed");
            }
            _ => (),
        }
    });
}

fn stop() {
    critical_section::with(|cs| {
        let mut player = PLAYER.borrow_ref_mut(cs);
        if player.state != PlaybackState::Stopped {
            audio_hardware_stop();
            player.state = PlaybackState::Stopped;
            info!("Playback stopped");
        }
    });
}

fn next() {
    critical_section::with(|cs| {
        let mut player = PLAYER.borrow_ref_mut(cs);
        let new_index = (player.current_track_index + 1) % PLAYLIST.len();
        player.current_track_index = new_index;
        info!("Switched to next track: {}", PLAYLIST[new_index].title);
    });

    if critical_section::with(|cs| PLAYER.borrow_ref(cs).state) == PlaybackState::Playing {
        let _ = play();
    }
}

fn prev() {
    critical_section::with(|cs| {
        let mut player = PLAYER.borrow_ref_mut(cs);
        let new_index = if player.current_track_index == 0 {
            PLAYLIST.len() - 1
        } else {
            player.current_track_index - 1
        };
        player.current_track_index = new_index;
        info!("Switched to previous track: {}", PLAYLIST[new_index].title);
    });
    if critical_section::with(|cs| PLAYER.borrow_ref(cs).state) == PlaybackState::Playing {
        let _ = play();
    }
}

fn volume_up() {
    let current = SPEAKER_VOLUME.load(core::sync::atomic::Ordering::Relaxed);
    let new = (current + 5).min(100);
    SPEAKER_VOLUME.store(new, core::sync::atomic::Ordering::Relaxed);
    info!("Media volume increased to {}%", new);
}

fn volume_down() {
    let current = SPEAKER_VOLUME.load(core::sync::atomic::Ordering::Relaxed);
    let new = current.saturating_sub(5);
    SPEAKER_VOLUME.store(new, core::sync::atomic::Ordering::Relaxed);
    info!("Media volume decreased to {}%", new);
}
