//! The game's one window: the game reads its size and the pointer from
//! [`Window`] and changes its title, mode and on-screen keyboard through
//! it; the runner keeps it and the system's window in step.

use std::sync::Arc;

use glam::{UVec2, Vec2};
pub use winit::window::CursorIcon as SystemCursorIcon;

/// The system's window, for drawing into.
#[derive(Clone)]
pub struct WindowHandle(pub Arc<winit::window::Window>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentMode {
    /// Waits for the display (a frame each refresh).
    #[default]
    AutoVsync,
    /// Draws as fast as it can.
    AutoNoVsync,
    Fifo,
    FifoRelaxed,
    Immediate,
    Mailbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSelection {
    Current,
    Primary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMode {
    #[default]
    Windowed,
    BorderlessFullscreen(MonitorSelection),
}

/// A window's size: physical pixels, and how many of them a logical pixel
/// takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowResolution {
    physical: UVec2,
    scale_factor: f32,
    scale_factor_override: Option<f32>,
    /// The size asked for when the window opens, in physical pixels; the
    /// system's default otherwise.
    requested: Option<UVec2>,
}

impl Default for WindowResolution {
    fn default() -> Self {
        WindowResolution {
            physical: UVec2::new(1280, 720),
            scale_factor: 1.0,
            scale_factor_override: None,
            requested: None,
        }
    }
}

impl WindowResolution {
    /// A window of `width`×`height` physical pixels.
    pub fn new(width: u32, height: u32) -> Self {
        let size = UVec2::new(width, height);
        WindowResolution {
            physical: size,
            requested: Some(size),
            ..Default::default()
        }
    }

    /// Takes a logical pixel to be `scale` physical ones whatever the
    /// system says.
    pub fn with_scale_factor_override(mut self, scale: f32) -> Self {
        self.scale_factor_override = Some(scale);
        self
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor_override.unwrap_or(self.scale_factor)
    }

    pub(crate) fn requested(&self) -> Option<UVec2> {
        self.requested
    }

    pub(crate) fn set_physical(&mut self, size: UVec2) {
        self.physical = size;
    }

    pub(crate) fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale;
    }
}

#[derive(Debug, Clone)]
pub struct Window {
    pub title: String,
    pub present_mode: PresentMode,
    pub resolution: WindowResolution,
    pub mode: WindowMode,
    /// Whether text input goes through the system's input method (on
    /// Android, whether its on-screen keyboard is up).
    pub ime_enabled: bool,
    pub focused: bool,
    /// The pointer, in physical pixels in the window, if it is over it.
    pub cursor: Option<Vec2>,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            title: "App".into(),
            present_mode: PresentMode::default(),
            resolution: WindowResolution::default(),
            mode: WindowMode::default(),
            ime_enabled: false,
            focused: true,
            cursor: None,
        }
    }
}

impl Window {
    pub fn physical_width(&self) -> u32 {
        self.resolution.physical.x
    }

    pub fn physical_height(&self) -> u32 {
        self.resolution.physical.y
    }

    /// The width in logical pixels.
    pub fn width(&self) -> f32 {
        self.resolution.physical.x as f32 / self.scale_factor()
    }

    pub fn height(&self) -> f32 {
        self.resolution.physical.y as f32 / self.scale_factor()
    }

    pub fn scale_factor(&self) -> f32 {
        self.resolution.scale_factor()
    }

    /// The pointer in logical pixels, if it is over the window.
    pub fn cursor_position(&self) -> Option<Vec2> {
        self.cursor.map(|p| p / self.scale_factor())
    }

    pub fn physical_cursor_position(&self) -> Option<Vec2> {
        self.cursor
    }

    /// Moves where the window thinks the pointer is (not the pointer).
    pub fn set_physical_cursor_position(&mut self, position: Option<Vec2>) {
        self.cursor = position;
    }
}

/// A picture for the pointer, in RGBA. Pictures with the same `key` and
/// size are taken to be the same.
#[derive(Debug, Clone)]
pub struct CustomCursor {
    pub key: u64,
    pub width: u16,
    pub height: u16,
    pub rgba: Arc<[u8]>,
    pub hotspot: (u16, u16),
}

impl PartialEq for CustomCursor {
    fn eq(&self, other: &Self) -> bool {
        (self.key, self.width, self.height, self.hotspot)
            == (other.key, other.width, other.height, other.hotspot)
    }
}

/// The pointer's look over the window.
#[derive(Debug, Clone, PartialEq)]
pub enum CursorIcon {
    System(SystemCursorIcon),
    Custom(CustomCursor),
}

impl Default for CursorIcon {
    fn default() -> Self {
        CursorIcon::System(SystemCursorIcon::Default)
    }
}
