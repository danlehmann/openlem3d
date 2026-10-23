//! What openlem3d needs from an engine: a window with its input (winit),
//! clocks, shared pictures, sound (rodio), colours, maths and logging, and
//! a loop that runs the game's frames. The shapes follow Bevy's, which the
//! game was first written against; only what it uses is here.

pub mod asset;
pub mod audio;
pub mod color;
mod engine;
pub mod image;
pub mod input;
mod logging;
pub mod math;
mod runner;
pub mod time;
pub mod window;

pub use engine::{Engine, Game, run};
pub use log;

/// The Android app the system started, kept by the entry point.
#[cfg(target_os = "android")]
pub mod android {
    use std::sync::OnceLock;

    pub use winit::platform::android::activity as android_activity;
    use winit::platform::android::activity::AndroidApp;

    pub static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

    pub fn app() -> &'static AndroidApp {
        ANDROID_APP
            .get()
            .expect("android_main keeps the Android app")
    }
}

pub mod prelude {
    pub use log::{debug, error, info, trace, warn};

    pub use crate::asset::{AssetEvent, AssetId, Assets, Handle};
    pub use crate::color::{Color, LinearRgba, Srgba};
    pub use crate::engine::{Engine, Game};
    pub use crate::image::Image;
    pub use crate::input::{ButtonInput, Input, KeyCode, MouseButton, Touch, Touches};
    pub use crate::math::*;
    pub use crate::time::{Clocks, Fixed, Real, Time, Virtual};
    pub use crate::window::Window;

    /// The default of whatever type is wanted.
    pub fn default<T: Default>() -> T {
        T::default()
    }
}
