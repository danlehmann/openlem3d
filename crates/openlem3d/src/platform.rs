//! What differs between platforms. Only Android does: it has no working
//! directory, environment or home directory to look in, so the game data and
//! settings live in the app's own storage; and the window, its insets and
//! the on-screen keyboard go through Java (in `android`).

use std::path::PathBuf;

#[cfg(target_os = "android")]
mod android;

/// Where the CD image lives when no `--data` was given, if the platform
/// dictates it (see `docs/GROUNDRULES.md`).
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    return android::data_dir();
    #[cfg(not(target_os = "android"))]
    None
}

/// The directory to keep `openlem3d/settings.txt` in, if the platform dictates
/// it.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    return android::app().internal_data_path();
    #[cfg(not(target_os = "android"))]
    None
}

/// How far the system draws over each edge of the window, in physical pixels:
/// left, top, right, bottom. On Android these are the caption bar of a
/// desktop window, the status and navigation bars, the on-screen keyboard,
/// and display cutouts unless the game fills the screen ([`full_screen`];
/// a camera hole then hides a little of the 3D view).
pub fn insets() -> [u32; 4] {
    #[cfg(target_os = "android")]
    return android::insets().unwrap_or_else(|e| {
        l3d_engine::warn_once!("could not read the window insets: {e}");
        [0; 4]
    });
    #[cfg(not(target_os = "android"))]
    [0; 4]
}

/// Makes the game fill the screen when it has the screen to itself
/// (Android: a phone or tablet, not a window among others as on a desktop, a
/// phone on a monitor, or in split screen): hides the status and navigation
/// bars, which a swipe from the edge brings back for a moment, and draws into
/// display cutouts. In a desktop's window the fullscreen setting decides
/// ([`window_fullscreen`]). Android may show the bars again when the game
/// loses focus, so this is done whenever it gains it.
pub fn full_screen() {
    #[cfg(target_os = "android")]
    android::full_screen();
}

/// Puts the game's window into full screen or back, for the fullscreen
/// setting where Bevy's window mode does nothing: an Android desktop's window
/// (a desktop, or a phone on a monitor), as on other desktops.
pub fn window_fullscreen(_on: bool) {
    #[cfg(target_os = "android")]
    android::window_fullscreen(_on);
}

/// Whether typing (a level code) brings up the system's on-screen keyboard:
/// on Android, where a device often has no other.
pub const ON_SCREEN_KEYBOARD: bool = cfg!(target_os = "android");

/// Brings up the on-screen keyboard again (Android): the player may have put
/// it away, and asking Bevy for it again does nothing while it thinks it is
/// still up.
pub fn show_keyboard() {
    #[cfg(target_os = "android")]
    android::app().show_soft_input(false);
}

/// Tells the user where to put the CD image.
#[cfg(target_os = "android")]
pub const DATA_HINT: &str = "Put the .cue and .bin in Android/media/org.openlem3d on the device.";
#[cfg(not(target_os = "android"))]
pub const DATA_HINT: &str =
    "Put the .cue and .bin in ./gamedata, set OPENLEM3D_DATA, or pass --data DIR.";
