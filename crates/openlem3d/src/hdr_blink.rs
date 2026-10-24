//! `--hdr-blink`, for comparing (ours, a test aid): every few seconds the
//! picture switches between HDR, glowing things above white, and the same
//! clipped at white as on an SDR screen, with a label saying which.

use l3d_engine::prelude::*;

use crate::World;
use crate::ui::{Node, NodeId, UiNode, percent};

/// How long each half lasts, in seconds.
const PERIOD: f32 = 5.0;
/// The label's canvas and size, as the mode notice's.
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const SCALE: f32 = 2.0;
// Below the HUD's key hints at the top.
const Y: f32 = 56.0;

/// The label shown, and whether it says HDR is on.
#[derive(Default)]
pub(crate) struct HdrBlink {
    label: Option<(NodeId, bool)>,
}

/// Whether HDR is on now: always, unless blinking and in the off half.
pub(crate) fn on(w: &World) -> bool {
    !w.opts.hdr_blink || ((w.e.time.real.elapsed_secs() / PERIOD) as u32).is_multiple_of(2)
}

/// Keeps the label saying whether HDR is on.
pub(crate) fn update(w: &mut World) {
    if !w.opts.hdr_blink {
        return;
    }
    let on = on(w);
    if let Some((id, shown)) = w.hdr_blink.label {
        if w.ui.contains(id) && shown == on {
            return;
        }
        w.ui.remove(id);
        w.hdr_blink.label = None;
    }
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let ui = &mut w.ui;
    let root = ui.root(UiNode::new().layout(Node {
        width: percent(100),
        height: percent(100),
        ..default()
    }));
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    let (glyphs, width) = font.layout(if on { "HDR on" } else { "HDR off" });
    let left = (SCREEN.x - width * SCALE) / 2.0;
    // A dark shadow, then the text.
    for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
        for (x, image) in &glyphs {
            let mut glyph = UiNode::image(image.clone()).at(
                left + x * SCALE + d,
                Y + d,
                font.size.x * SCALE,
                font.size.y * SCALE,
            );
            if let Some(c) = shade {
                glyph = glyph.tint(c);
            }
            ui.child(canvas, glyph);
        }
    }
    w.hdr_blink.label = Some((root, on));
}
