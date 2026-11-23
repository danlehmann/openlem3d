//! Player settings, kept in a small `key = value` file in the user's
//! configuration folder (`%APPDATA%\openlem3d\settings.txt` on Windows,
//! `$XDG_CONFIG_HOME/openlem3d` or `~/.config/openlem3d` elsewhere).

use std::path::PathBuf;

use l3d_engine::prelude::*;

/// What the options screen changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub fullscreen: bool,
    /// How the game is presented: as the original, or with our
    /// improvements (`Mode`).
    pub mode: Mode,
    /// Enhanced's graphics (ours).
    pub graphics: Graphics,
    /// They suit the machine: chosen by `autodetect`, or by hand.
    pub calibrated: bool,
    /// Show the frames per second in a corner (ours).
    pub fps: bool,
    /// Backspace runs the level back in time (ours); off, no snapshots are
    /// kept for it.
    pub rewind: bool,
    /// Volumes and camera speed, 0–10 lights as on the original's sliders.
    pub music: u8,
    pub effects: u8,
    pub camera: u8,
    /// The rating last picked on the title screen (`menu::RATINGS`).
    pub rating: usize,
    /// Levels completed (file numbers), for the Practice menu's ticks.
    pub completed: std::collections::BTreeSet<u32>,
    /// Best result per level (file number): lemmings saved, and seconds left
    /// on the clock when it was completed (0 if never).
    pub best: std::collections::BTreeMap<u32, (u32, u32)>,
}

/// How the game is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// As the original: its 3D view at 320×200, its camera cuts.
    Original,
    /// Ours: full resolution, smooth camera moves, and the effects its
    /// [`Graphics`] choose.
    Enhanced,
}

impl Mode {
    /// Its name as shown.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Original => "Original",
            Mode::Enhanced => "Enhanced",
        }
    }

    /// Its name in the settings file.
    fn key(self) -> &'static str {
        match self {
            Mode::Original => "original",
            Mode::Enhanced => "enhanced",
        }
    }

    /// The mode named `key`, and for the Ultra there once was, the High
    /// preset it now is.
    pub fn from_name(key: &str) -> Option<(Self, Option<Preset>)> {
        match key {
            "original" => Some((Mode::Original, None)),
            "enhanced" => Some((Mode::Enhanced, None)),
            "ultra" => Some((Mode::Enhanced, Some(Preset::High))),
            _ => None,
        }
    }

    /// The other mode.
    pub fn next(self) -> Self {
        match self {
            Mode::Original => Mode::Enhanced,
            Mode::Enhanced => Mode::Original,
        }
    }
}

/// A set of Enhanced's graphics settings, for weaker or stronger machines;
/// Custom once one of them is changed by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Low,
    Medium,
    High,
    Custom,
}

impl Preset {
    pub const CHOICES: [Preset; 3] = [Preset::Low, Preset::Medium, Preset::High];

    pub fn name(self) -> &'static str {
        match self {
            Preset::Low => "Low",
            Preset::Medium => "Medium",
            Preset::High => "High",
            Preset::Custom => "Custom",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Preset::Low => "low",
            Preset::Medium => "medium",
            Preset::High => "high",
            Preset::Custom => "custom",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        [Preset::Low, Preset::Medium, Preset::High, Preset::Custom]
            .into_iter()
            .find(|p| p.key() == key)
    }

    /// Its settings (Custom keeps `current`'s).
    pub fn graphics(self, current: Graphics) -> Graphics {
        let base = self.graphics_base();
        match self {
            Preset::High => base,
            Preset::Medium => Graphics {
                reflections: false,
                glow: Glow::On,
                ..base
            },
            Preset::Low => Graphics {
                resolution: 2,
                filtering: false,
                waves: false,
                reflections: false,
                shadows: false,
                glow: Glow::Off,
                snow: false,
                sandstorm: false,
                crt: false,
                windows: false,
                ..base
            },
            Preset::Custom => Graphics {
                preset: Preset::Custom,
                ..current
            },
        }
    }
}

/// Enhanced's glow: none; glowing things (flames, lamps) drawn bright and
/// lighting the walls near them; and that glow blooming softly round them
/// too, which costs a weak GPU much.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    Off,
    On,
    Bloom,
}

impl Glow {
    pub fn name(self) -> &'static str {
        match self {
            Glow::Off => "Off",
            Glow::On => "On",
            Glow::Bloom => "Bloom",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Glow::Off => Glow::On,
            Glow::On => Glow::Bloom,
            Glow::Bloom => Glow::Off,
        }
    }

    pub fn previous(self) -> Self {
        self.next().next()
    }

    fn key(self) -> &'static str {
        match self {
            Glow::Off => "off",
            Glow::On => "on",
            Glow::Bloom => "bloom",
        }
    }
}

/// Enhanced's graphics (ours): a [`Preset`]'s, or chosen one by one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graphics {
    pub preset: Preset,
    /// The resolution drawn at, as a step of [`RESOLUTIONS`]: 0 the
    /// screen's own, then fewer rows, stretched over it.
    pub resolution: u8,
    /// Distant textures sampled smoothly (mipmaps, anisotropic), rather
    /// than nearest.
    pub filtering: bool,
    /// The sea's waves and surf.
    pub waves: bool,
    /// The level mirrored in the sea, and polished floors shining (only
    /// with `waves`).
    pub reflections: bool,
    /// Soft shadows under the lemmings.
    pub shadows: bool,
    pub glow: Glow,
    /// Snow falling and lying on snowy levels.
    pub snow: bool,
    /// Sandstorms (and heat shimmer) on desert levels.
    pub sandstorm: bool,
    /// Screens in the levels drawn as cathode-ray tubes.
    pub crt: bool,
    /// Windows looking out on space (each of their pixels follows the line
    /// of sight through the level).
    pub windows: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Preset::High.graphics_base()
    }
}

impl Preset {
    /// High's settings, named `self`.
    fn graphics_base(self) -> Graphics {
        Graphics {
            preset: self,
            resolution: 0,
            filtering: true,
            waves: true,
            reflections: true,
            shadows: true,
            glow: Glow::Bloom,
            snow: true,
            sandstorm: true,
            crt: true,
            windows: true,
        }
    }
}

/// The resolutions offered: fractions of the screen's rows. On a TV they
/// are named by rows, as TVs are (1080p, 720p, 540p on a 1080-row
/// screen); elsewhere, where windows take any size, as percentages.
const RESOLUTIONS_TV: [f32; 3] = [1.0, 2.0 / 3.0, 0.5];
const RESOLUTIONS_WINDOWED: [f32; 3] = [1.0, 0.75, 0.5];
pub const RESOLUTIONS: usize = 3;

/// The fraction of the screen's rows resolution step `step` draws.
pub fn resolution(step: u8) -> f32 {
    let steps = if crate::platform::is_tv() {
        RESOLUTIONS_TV
    } else {
        RESOLUTIONS_WINDOWED
    };
    steps[(step as usize).min(RESOLUTIONS - 1)]
}

/// Resolution step `step`'s name on a screen of `rows` rows: on a TV its
/// rows ("720p"), elsewhere a percentage.
pub fn resolution_name(step: u8, rows: u32) -> String {
    let f = resolution(step);
    if crate::platform::is_tv() {
        format!("{}p", (rows as f32 * f).round() as u32)
    } else {
        format!("{}%", (f * 100.0).round() as u32)
    }
}

/// Steps on each slider.
pub const SLIDER_STEPS: u8 = 10;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fullscreen: false,
            mode: Mode::Enhanced,
            // This machine's guess, once its GPU is known (so Default
            // Config gives it too).
            graphics: crate::autodetect::default_preset().graphics(Graphics::default()),
            calibrated: false,
            fps: false,
            rewind: true,
            music: 8,
            effects: 8,
            camera: 5,
            rating: 0,
            completed: Default::default(),
            best: Default::default(),
        }
    }
}

impl Settings {
    /// Records a finished level; returns whether anything improved. Only a
    /// level passed counts (owner: one failed showed its lemmings saved).
    pub fn record(&mut self, level: u32, saved: u32, seconds_left: u32, passed: bool) -> bool {
        if !passed {
            return false;
        }
        let old = self.best.get(&level).copied().unwrap_or((0, 0));
        let new = (old.0.max(saved), old.1.max(seconds_left));
        let completed = self.completed.insert(level);
        if new != old {
            self.best.insert(level, new);
        }
        completed || new != old
    }

    /// Whether the game is presented with our improvements (Enhanced),
    /// rather than as the original.
    pub fn enhanced(&self) -> bool {
        self.mode != Mode::Original
    }

    /// Enhanced's graphics, or none of them in Original.
    pub fn effects(&self) -> Option<Graphics> {
        self.enhanced().then_some(self.graphics)
    }

    /// The scene parts and style to draw.
    pub fn show(&self) -> crate::scene_build::Show {
        crate::scene_build::Show {
            land: true,
            sea: true,
            sky: true,
            bottom_sides: self.enhanced(),
            waves: self.effects().is_some_and(|g| g.waves),
            // The mirror is made with the waves (and polished floors shine
            // with it).
            mirror: self.effects().is_some_and(|g| g.reflections && g.waves),
            storm: self.effects().is_some_and(|g| g.sandstorm),
            snow: self.effects().is_some_and(|g| g.snow),
            windows: self.effects().is_some_and(|g| g.windows),
            signals: self.enhanced(),
        }
    }

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
        MouseButton::Left
    }

    /// The mouse button that turns the camera when dragged.
    pub fn turn_button(&self) -> MouseButton {
        MouseButton::Right
    }

    fn path() -> Option<PathBuf> {
        let base = crate::platform::config_dir()
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("openlem3d").join("settings.txt"))
    }

    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = Self::path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            let flag = v == "on";
            let level = v.parse::<u8>().ok().map(|n| n.min(SLIDER_STEPS));
            match k {
                "fullscreen" => s.fullscreen = flag,
                "mode" => {
                    if let Some((mode, preset)) = Mode::from_name(v) {
                        s.mode = mode;
                        if let Some(p) = preset {
                            s.graphics = p.graphics(s.graphics);
                        }
                    }
                }
                "preset" => {
                    if let Some(p) = Preset::from_key(v) {
                        s.graphics.preset = p;
                    }
                }
                "resolution" => {
                    s.graphics.resolution = v
                        .parse()
                        .ok()
                        .filter(|&r: &u8| (r as usize) < RESOLUTIONS)
                        .unwrap_or(s.graphics.resolution)
                }
                "filtering" => s.graphics.filtering = flag,
                "waves" => s.graphics.waves = flag,
                "reflections" => s.graphics.reflections = flag,
                "shadows" => s.graphics.shadows = flag,
                "glow" => {
                    s.graphics.glow = match v {
                        "off" => Glow::Off,
                        "on" => Glow::On,
                        "bloom" => Glow::Bloom,
                        _ => s.graphics.glow,
                    }
                }
                "snow" => s.graphics.snow = flag,
                "sandstorm" => s.graphics.sandstorm = flag,
                "crt" => s.graphics.crt = flag,
                // Before CRT and windows were apart.
                "screens" => (s.graphics.crt, s.graphics.windows) = (flag, flag),
                "windows" => s.graphics.windows = flag,
                "calibrated" => s.calibrated = flag,
                // Before there were three modes.
                "enhanced" => s.mode = if flag { Mode::Enhanced } else { Mode::Original },
                "fps" => s.fps = flag,
                "rewind" => s.rewind = flag,
                "music" => s.music = level.unwrap_or(s.music),
                "effects" => s.effects = level.unwrap_or(s.effects),
                "camera" => s.camera = level.unwrap_or(s.camera),
                "rating" => {
                    s.rating = v
                        .parse()
                        .ok()
                        .filter(|&r| r < crate::menu::RATINGS.len())
                        .unwrap_or(s.rating)
                }
                "completed" => {
                    s.completed = v.split(',').filter_map(|n| n.trim().parse().ok()).collect()
                }
                // best = level:saved:seconds, …
                "best" => {
                    s.best = v
                        .split(',')
                        .filter_map(|e| {
                            let p: Vec<u32> =
                                e.split(':').filter_map(|n| n.trim().parse().ok()).collect();
                            (p.len() == 3).then(|| (p[0], (p[1], p[2])))
                        })
                        .collect()
                }
                _ => {}
            }
        }
        // Results kept for levels never passed (before only passes were
        // recorded) are dropped.
        let completed = s.completed.clone();
        s.best.retain(|level, _| completed.contains(level));
        s
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let on = |b: bool| if b { "on" } else { "off" };
        let text = format!(
            "fullscreen = {}\nmode = {}\npreset = {}\nresolution = {}\nfiltering = {}\nwaves = {}\nreflections = {}\nshadows = {}\nglow = {}\nsnow = {}\nsandstorm = {}\ncrt = {}\nwindows = {}\ncalibrated = {}\nfps = {}\nrewind = {}\nmusic = {}\neffects = {}\ncamera = {}\nrating = {}\ncompleted = {}\nbest = {}\n",
            on(self.fullscreen),
            self.mode.key(),
            self.graphics.preset.key(),
            self.graphics.resolution,
            on(self.graphics.filtering),
            on(self.graphics.waves),
            on(self.graphics.reflections),
            on(self.graphics.shadows),
            self.graphics.glow.key(),
            on(self.graphics.snow),
            on(self.graphics.sandstorm),
            on(self.graphics.crt),
            on(self.graphics.windows),
            on(self.calibrated),
            on(self.fps),
            on(self.rewind),
            self.music,
            self.effects,
            self.camera,
            self.rating,
            self.completed
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
            self.best
                .iter()
                .map(|(l, (s, t))| format!("{l}:{s}:{t}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, text) {
            warn!("could not save settings to {}: {e}", path.display());
        }
    }
}
