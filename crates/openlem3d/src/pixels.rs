//! The 3D view's resolution: the original's 320×200 in Original mode,
//! stretched over the screen in blocks as the original's VGA picture was
//! (`docs/spec/ui-graphics.md`, "Resolution"); the screen's own in Enhanced
//! mode. Switching modes during a level sharpens the blocks step by step
//! into the detailed picture, or coarsens it back (ours); the level waits
//! meanwhile.

use l3d_engine::prelude::*;

use crate::World;

/// The original's picture: 320×200 pixels filling a 4:3 screen.
const ORIGINAL: Vec2 = Vec2::new(320.0, 200.0);
const ORIGINAL_ASPECT: f32 = 4.0 / 3.0;
/// How long a change of resolution takes, seconds, and in how many steps.
const CHANGE_SECONDS: f32 = 1.2;
const CHANGE_STEPS: f32 = 10.0;

/// Where the view is between the original's resolution (0) and the
/// screen's (1), and the change under way, if any.
pub struct Pixelation {
    detail: f32,
    change: Option<Change>,
    /// The mode last seen, to notice a switch.
    enhanced: Option<bool>,
}

impl Default for Pixelation {
    fn default() -> Self {
        Pixelation {
            detail: 1.0,
            change: None,
            enhanced: None,
        }
    }
}

impl Pixelation {
    /// Whether the resolution is changing; the level waits meanwhile.
    pub fn changing(&self) -> bool {
        self.change.is_some()
    }
}

/// A change of resolution: from which detail, towards which, and when it
/// started.
struct Change {
    from: f32,
    to: f32,
    start: f32,
}

pub(crate) fn update(w: &mut World) {
    let now = w.e.time.real.elapsed_secs();
    let settings = &w.settings;
    let target = if settings.enhanced { 1.0 } else { 0.0 };
    let p = &mut w.pixelation;
    let camera = &mut w.camera;
    if p.enhanced != Some(settings.enhanced) {
        // A switch during a level is shown; elsewhere (and at start) the
        // new resolution applies at once.
        if p.enhanced.is_some() && w.state == crate::menu::AppState::Playing {
            p.change = Some(Change {
                from: p.detail,
                to: target,
                start: now,
            });
        } else {
            p.detail = target;
            p.change = None;
        }
        p.enhanced = Some(settings.enhanced);
    }
    if w.state != crate::menu::AppState::Playing {
        // Leaving the level finishes a change at once.
        if let Some(c) = p.change.take() {
            p.detail = c.to;
        }
    }
    if let Some(c) = &p.change {
        // In whole steps, so each stage of blocks holds for a moment.
        let t = ((now - c.start) / CHANGE_SECONDS).clamp(0.0, 1.0);
        let t = (t * CHANGE_STEPS).floor() / CHANGE_STEPS;
        p.detail = c.from + (c.to - c.from) * t;
        if now - c.start >= CHANGE_SECONDS {
            p.detail = c.to;
            p.change = None;
        }
    }
    camera.clear = w.clear.0.to_linear().to_vec4();
    camera.glow = settings.enhanced && settings.glow && !w.opts.no_glow;
    camera.filtered = settings.enhanced;
    let screen = (w.screen.size * w.e.window.scale_factor()).round();
    camera.pixels = if p.detail >= 1.0 || screen.min_element() < 1.0 {
        UVec2::ZERO
    } else {
        // The original's rows, and as many columns as keep its pixel shape
        // on this screen; then towards the screen's size geometrically, so
        // the blocks halve at an even pace.
        let low = Vec2::new(
            (ORIGINAL.x * (screen.x / screen.y) / ORIGINAL_ASPECT).round(),
            ORIGINAL.y,
        )
        .min(screen);
        (low * (screen / low).powf(p.detail))
            .round()
            .max(Vec2::ONE)
            .as_uvec2()
    };
}
