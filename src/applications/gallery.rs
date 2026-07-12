// APPLICATIONS/GALLERY
// DRAWS IMAGES FROM SDCARD
// SWIPE LEFT/RIGHT BETWEEN THEM

// ───────────────────────────────────────────────────────────────────────
// DESCRIBE THIS APPLICATION
pub const APP_DESCRIPTOR: crate::applications::AppDescriptor = crate::applications::AppDescriptor {
    name: "gallery",
    description: "Display all images in the /share directory of the local storage.",
    launch: open_app,
    icon: crate::base::assets::GALLERY_PNG,
};

pub fn open_app() { 
    crate::components::storage::ensure_sd_ready().ok();
    load_gallery_images();
    crate::store!(crate::gui::pages::CURRENT_PAGE, crate::gui::pages::Page::Gallery as u8);     
}

// ───────────────────────────────────────────────────────────────────────

static GALLERY_IMAGES: critical_section::Mutex<core::cell::RefCell<alloc::vec::Vec<alloc::string::String>>> = 
    critical_section::Mutex::new(core::cell::RefCell::new(alloc::vec::Vec::new()));

pub fn load_gallery_images() {
    let entries = match crate::components::storage::list_dir("/share/share") {
        Ok(list) => list,
        Err(e) => {
            defmt::error!("Gallery: failed to list /share: {:?}", e);
            return;
        }
    };

    let mut png_files: alloc::vec::Vec<alloc::string::String> = entries
        .into_iter()
        .filter(|(name, is_dir, _)| !is_dir && name.to_lowercase().ends_with(".png"))
        .map(|(name, _, _)| name)
        .collect();

    png_files.sort();
    defmt::debug!("Gallery: found {} PNG images", png_files.len());

    critical_section::with(|cs| {
        *GALLERY_IMAGES.borrow(cs).borrow_mut() = png_files;
    });

    let count = gallery_image_count();
    if crate::load!(crate::state::GALLERY_INDEX) >= count && count > 0 {
        crate::store!(crate::state::GALLERY_INDEX, 0u8);
    }
}

pub fn gallery_image_count() -> u8 {
    critical_section::with(|cs| {
        GALLERY_IMAGES.borrow(cs).borrow().len() as u8
    })
}

pub fn gallery_image_at(index: u8) -> Option<alloc::string::String> {
    critical_section::with(|cs| {
        GALLERY_IMAGES.borrow(cs).borrow().get(index as usize).cloned()
    })
}
