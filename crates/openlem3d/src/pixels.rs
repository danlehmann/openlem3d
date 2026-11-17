//! The 3D view's resolution: the original's 320×200 in Original mode,
//! stretched over the screen in blocks as the original's VGA picture was
//! (`docs/spec/ui-graphics.md`, "Resolution"); the screen's own in the
//! other modes.

use l3d_engine::prelude::*;

use crate::World;

/// The original's picture: 320×200 pixels filling a 4:3 screen.
const ORIGINAL: Vec2 = Vec2::new(320.0, 200.0);
const ORIGINAL_ASPECT: f32 = 4.0 / 3.0;

pub(crate) fn update(w: &mut World) {
    let settings = &w.settings;
    let camera = &mut w.camera;
    camera.clear = w.clear.0.to_linear().to_vec4();
    camera.glow = settings.mode == crate::settings::Mode::Ultra && !w.opts.no_glow;
    camera.filtered = settings.enhanced();
    camera.crt = settings.enhanced();
    let screen = (w.screen.size * w.e.window.scale_factor()).round();
    camera.pixels = if settings.enhanced() || screen.min_element() < 1.0 {
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
