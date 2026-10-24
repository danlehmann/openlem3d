//! What differs between platforms. Only Android does: it has no working
//! directory, environment or home directory to look in, so the game data and
//! settings live in the app's own storage; and the window, its insets and
//! the on-screen keyboard go through Java (in `android`).

use std::path::PathBuf;

#[cfg(target_os = "android")]
mod android;

/// The command-line arguments, without the program's name. Android starts
/// apps without any, so there they are the words of `args.txt` in the app's
/// internal storage, if it is there (put there with adb's `run-as`, for
/// testing).
pub fn args() -> Vec<String> {
    #[cfg(target_os = "android")]
    return android::app()
        .internal_data_path()
        .and_then(|d| std::fs::read_to_string(d.join("args.txt")).ok())
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    #[cfg(not(target_os = "android"))]
    std::env::args().skip(1).collect()
}

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

/// How far above white to draw, as a multiple of white: on macOS what the
/// screen can show now (its extended dynamic range headroom, which follows
/// the brightness); on Android a fixed amount the system fits to the
/// display; 1 elsewhere.
pub fn hdr_headroom() -> f32 {
    #[cfg(target_os = "macos")]
    return objc2::MainThreadMarker::new()
        .and_then(objc2_app_kit::NSScreen::mainScreen)
        .map_or(1.0, |s| {
            s.maximumExtendedDynamicRangeColorComponentValue().max(1.0) as f32
        });
    // The headroom the HDR layer is drawn with, which Android fits to the
    // display.
    #[cfg(target_os = "android")]
    return android::HDR_HEADROOM;
    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    1.0
}

/// Where HDR takes a layer of its own to draw into rather than the window
/// (Android: see `android::HdrLayer`); elsewhere there is never one.
pub struct HdrLayer {
    #[cfg(target_os = "android")]
    layer: android::HdrLayer,
    /// Its size in pixels.
    pub size: l3d_engine::prelude::UVec2,
}

impl HdrLayer {
    /// A layer of `size` pixels, where HDR can be shown through one (on
    /// Android once its view is ready; the first call adds it).
    pub fn create(size: l3d_engine::prelude::UVec2) -> Option<HdrLayer> {
        #[cfg(target_os = "android")]
        return android::HdrLayer::create()
            .unwrap_or_else(|e| {
                l3d_engine::warn_once!("could not make an HDR layer: {e}");
                None
            })
            .map(|layer| HdrLayer { layer, size });
        #[cfg(not(target_os = "android"))]
        {
            let _ = size;
            None
        }
    }

    /// Gets ready to make layers (Android: adds the view, the first time).
    pub fn prepare() {
        #[cfg(target_os = "android")]
        if let Err(e) = android::HdrLayer::prepare() {
            l3d_engine::warn_once!("could not prepare an HDR layer: {e}");
        }
    }

    /// Gives up on HDR layers: one was made, but can't show HDR.
    pub fn give_up() {
        #[cfg(target_os = "android")]
        android::HdrLayer::give_up();
    }

    /// Whether a layer could be made now where there was none before.
    pub fn ready() -> bool {
        #[cfg(target_os = "android")]
        return android::HdrLayer::ready();
        #[cfg(not(target_os = "android"))]
        false
    }

    /// What to make a GPU surface from.
    pub fn target(&self) -> wgpu::SurfaceTargetUnsafe {
        #[cfg(target_os = "android")]
        return wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Android(
                wgpu::rwh::AndroidDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::AndroidNdk(
                wgpu::rwh::AndroidNdkWindowHandle::new(self.layer.window()),
            ),
        };
        #[cfg(not(target_os = "android"))]
        unreachable!("only Android makes HDR layers")
    }
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
