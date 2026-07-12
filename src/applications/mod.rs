// APPLICATIONS/MOD

// ───────────────────────────────────────────────────────────────────────
// LOAD APP MODULES
pub mod duck_tv;
pub mod caller;
pub mod settings;
pub mod tinyweather;
pub mod zigduck;


// ───────────────────────────────────────────────────────────────────────
// DESCRIBES AN APPLICATION
pub struct AppDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub launch: fn(),
    pub icon: &'static [u8],
}

// ───────────────────────────────────────────────────────────────────────
// FETCH ALL APPDESCRIPTIORS
// APP LAUNCHER LIST APPS IN SAME ORDER AS THEY'RE LISTED HERE!
pub static APPS: &[AppDescriptor] = &[
    duck_tv::APP_DESCRIPTOR,
    caller::APP_DESCRIPTOR,
    tinyweather::APP_DESCRIPTOR,
    zigduck::APP_DESCRIPTOR,
    settings::APP_DESCRIPTOR,
];    
