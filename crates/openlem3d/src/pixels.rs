//! The 3D view's resolution: the original's 320×200 in Original mode,
//! stretched over the screen in blocks as the original's VGA picture was
//! (`docs/spec/ui-graphics.md`, "Resolution"); the screen's own in the
//! other modes.

use l3d_engine::prelude::*;

use crate::World;
use crate::settings::Glow;

/// The original's picture: 320×200 pixels filling a 4:3 screen.
const ORIGINAL: Vec2 = Vec2::new(320.0, 200.0);
const ORIGINAL_ASPECT: f32 = 4.0 / 3.0;

pub(crate) fn update(w: &mut World) {
    let settings = &w.settings;
    let camera = &mut w.camera;
    camera.clear = w.clear.0.to_linear().to_vec4();
    let effects = settings.effects();
    let glow = effects.map_or(Glow::Off, |g| g.glow);
    camera.glow = glow != Glow::Off && !w.opts.no_glow;
    camera.bloom = glow == Glow::Bloom;
    camera.filtered = effects.is_some_and(|g| g.filtering);
    // Screens as cathode-ray tubes: always in Enhanced (it costs nothing).
    camera.crt = effects.is_some();
    camera.ours = settings.enhanced();
    // Enhanced's resolution, in levels (the menus are cheap, and stay
    // sharp): fewer rows drawn, the system stretching them over the window
    // (`--max-height` overrides it).
    let in_level = w.state == crate::menu::AppState::Playing;
    let fraction = effects
        .filter(|_| in_level)
        .map_or(1.0, |g| crate::settings::resolution(g.resolution));
    let window = &mut w.e.window.resolution;
    let rows = w.opts.max_height.or_else(|| {
        (fraction < 1.0).then(|| (window.actual_height() as f32 * fraction).round() as u32)
    });
    if window.max_height() != rows {
        window.set_max_height(rows);
    }
    let screen = (w.screen.size * w.e.window.scale_factor()).round();
    // Enhanced (or no screen yet): the window's own size (zero).
    camera.pixels = if screen.min_element() < 1.0 || settings.enhanced() {
        UVec2::ZERO
    } else {
        // The original's rows, and as many columns as keep its pixel shape
        // on this screen.
        Vec2::new(
            (ORIGINAL.x * (screen.x / screen.y) / ORIGINAL_ASPECT).round(),
            ORIGINAL.y,
        )
        .min(screen)
        .max(Vec2::ONE)
        .as_uvec2()
    };
}
