//! The part of the window the game uses: all of it, except where the system
//! draws over it, such as the caption bar of an Android desktop window
//! (`platform::insets`). The scene and the UI are drawn in just this part;
//! code that measures the screen or places things by the pointer uses
//! [`Screen`] rather than the window.

use l3d_engine::prelude::*;

use crate::World;

/// The usable part of the window, in logical pixels.
#[derive(Clone, Copy)]
pub struct Screen {
    /// Its top-left corner in the window.
    pub min: Vec2,
    pub size: Vec2,
    /// The same in physical pixels: left, top, width, height.
    pub pixels: UVec4,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            min: Vec2::ZERO,
            size: Vec2::ONE,
            pixels: UVec4::new(0, 0, 1, 1),
        }
    }
}

impl Screen {
    /// A window position (the cursor's or a touch's) relative to the screen.
    pub fn on_screen(&self, p: Vec2) -> Vec2 {
        p - self.min
    }

    /// The cursor's position on the screen, if it is in the window.
    pub fn cursor(&self, window: &Window) -> Option<Vec2> {
        window.cursor_position().map(|p| self.on_screen(p))
    }
}

/// Fills the screen where the game has it to itself (see
/// `platform::full_screen`) whenever the window gains focus.
pub(crate) fn full_screen(w: &mut World) {
    if w.e.input.focus.contains(&true) {
        crate::platform::full_screen();
    }
}

/// How often to ask the system for the insets, seconds; they change only when
/// the window does, but asking is not free on Android.
const INSETS_POLL: f32 = 0.25;

pub(crate) fn update(w: &mut World) {
    let window = &w.e.window;
    let now = w.e.time.real.elapsed_secs();
    let insets = &mut w.insets;
    if insets.is_none_or(|(at, _)| now - at >= INSETS_POLL) {
        *insets = Some((now, crate::platform::insets()));
    }
    let [left, top, right, bottom] = insets.map_or([0; 4], |(_, i)| i);
    let screen = &mut w.screen;
    let full = UVec2::new(window.physical_width(), window.physical_height());
    let min = UVec2::new(left, top).min(full);
    let size = (full - min)
        .saturating_sub(UVec2::new(right, bottom))
        .max(UVec2::ONE);
    let scale = window.scale_factor();
    let new = Screen {
        min: min.as_vec2() / scale,
        size: size.as_vec2() / scale,
        pixels: UVec4::new(min.x, min.y, size.x, size.y),
    };
    if new.min != screen.min || new.size != screen.size || new.pixels != screen.pixels {
        *screen = new;
    }
}
