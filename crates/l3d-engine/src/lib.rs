//! What openlem3d needs around its ECS (Bevy's): an app with schedules and
//! plugins, states, clocks, a window with its input (winit), shared
//! pictures, sound (rodio), colours, maths and logging. The shapes follow
//! Bevy's, which the game was written against; only what it uses is here.

pub mod app;
pub mod asset;
pub mod audio;
pub mod color;
pub mod image;
pub mod input;
mod logging;
pub mod math;
mod runner;
pub mod state;
pub mod time;
pub mod window;

pub use bevy_ecs as ecs;
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
    pub use bevy_ecs::prelude::*;
    pub use log::{debug, error, info, trace, warn};

    pub use crate::app::{
        App, AppExit, First, FixedUpdate, Last, Plugin, PostUpdate, PreUpdate, Startup, Update,
    };
    pub use crate::asset::{AssetEvent, AssetId, Assets, Handle};
    pub use crate::color::{Color, LinearRgba, Srgba};
    pub use crate::image::Image;
    pub use crate::input::{ButtonInput, KeyCode, MouseButton, Touch, Touches};
    pub use crate::math::*;
    pub use crate::state::{NextState, OnEnter, OnExit, OnTransition, State, States, in_state};
    pub use crate::time::{Fixed, Real, Time, Virtual};
    pub use crate::window::{PrimaryWindow, Window};

    /// The default of whatever type is wanted.
    pub fn default<T: Default>() -> T {
        T::default()
    }
}
