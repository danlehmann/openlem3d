//! Player settings, kept in a small `key = value` file in the user's
//! configuration folder (`%APPDATA%\openlem3d\settings.txt` on Windows,
//! `$XDG_CONFIG_HOME/openlem3d` or `~/.config/openlem3d` elsewhere).

use std::path::PathBuf;

use bevy::prelude::*;

/// What the options screen changes.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Draw the land, sea and sky (off: plain colour), as the original's
    /// configuration offers.
    pub land: bool,
    pub sea: bool,
    pub sky: bool,
    /// Swap the mouse buttons: the right one gives skills, the left one
    /// turns the camera.
    pub left_handed: bool,
    pub fullscreen: bool,
    /// Volumes and camera speed, 0–10 lights as on the original's sliders.
    pub music: u8,
    pub effects: u8,
    pub camera: u8,
    /// Levels completed (file numbers), for the Practice menu's ticks.
    pub completed: std::collections::BTreeSet<u32>,
}

/// Steps on each slider.
pub const SLIDER_STEPS: u8 = 10;

impl Default for Settings {
    fn default() -> Self {
        Settings { land: true, sea: true, sky: true, left_handed: false, fullscreen: false, music: 8, effects: 8, camera: 5, completed: Default::default() }
    }
}

impl Settings {
    /// Music volume as a linear factor.
    pub fn music_volume(&self) -> f32 {
        0.75 * self.music as f32 / SLIDER_STEPS as f32
    }

    pub fn effects_volume(&self) -> f32 {
        self.effects as f32 / SLIDER_STEPS as f32
    }

    /// Camera speed relative to the default.
    pub fn camera_factor(&self) -> f32 {
        (self.camera.max(1) as f32 / 5.0).powf(0.8)
    }

    /// The mouse button that gives skills and picks lemmings.
    pub fn action_button(&self) -> MouseButton {
        if self.left_handed { MouseButton::Right } else { MouseButton::Left }
    }

    /// The mouse button that turns the camera when dragged.
    pub fn turn_button(&self) -> MouseButton {
        if self.left_handed { MouseButton::Left } else { MouseButton::Right }
    }

    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("openlem3d").join("settings.txt"))
    }

    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = Self::path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let flag = v == "on";
            let level = v.parse::<u8>().ok().map(|n| n.min(SLIDER_STEPS));
            match k {
                "land" => s.land = flag,
                "sea" => s.sea = flag,
                "sky" => s.sky = flag,
                "left_handed" => s.left_handed = flag,
                "fullscreen" => s.fullscreen = flag,
                "music" => s.music = level.unwrap_or(s.music),
                "effects" => s.effects = level.unwrap_or(s.effects),
                "camera" => s.camera = level.unwrap_or(s.camera),
                "completed" => s.completed = v.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let on = |b: bool| if b { "on" } else { "off" };
        let text = format!(
            "land = {}\nsea = {}\nsky = {}\nleft_handed = {}\nfullscreen = {}\nmusic = {}\neffects = {}\ncamera = {}\ncompleted = {}\n",
            on(self.land),
            on(self.sea),
            on(self.sky),
            on(self.left_handed),
            on(self.fullscreen),
            self.music,
            self.effects,
            self.camera,
            self.completed.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        );
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, text) {
            warn!("could not save settings to {}: {e}", path.display());
        }
    }
}
