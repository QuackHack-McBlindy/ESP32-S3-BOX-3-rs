// APPLICATIONS/CALLER
// CALL HOME

// ───────────────────────────────────────────────────────────────────────
// DESCRIBE THIS APPLICATION
pub const APP_DESCRIPTOR: crate::applications::AppDescriptor = crate::applications::AppDescriptor {
    name: "caller",
    description: "Make a phone call home with bidirectional audio.",
    launch: open_app,
    icon: crate::base::assets::CALL_ACCEPT_PNG,
};

pub fn open_app() {
    defmt::info!("Opening tinyWeather app");
    crate::store!(crate::gui::pages::CURRENT_PAGE, crate::gui::pages::Page::Call as u8);
}

// ───────────────────────────────────────────────────────────────────────
// 
