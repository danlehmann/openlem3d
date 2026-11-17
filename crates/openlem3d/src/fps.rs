//! The frames-per-second counter (ours): F10 shows or hides it, on every
//! screen, and the setting is kept.
//!
//! It is laid out in screen pixels, not on a canvas: the UI scales every
//! node by the first canvas it finds, and a canvas of the counter's own
//! would rescale the screen beneath it.

use l3d_engine::prelude::*;

use crate::World;
use crate::ui::{Node, NodeId, UiNode, percent, px};

/// The screen's height in the counter's pixels (its glyphs' size), and how
/// far down the screen the text's top is in them (centred across, under
/// the level's top line).
const ROWS: f32 = 480.0;
const TOP: f32 = 40.0;
/// How long frames are counted for each figure, in seconds.
const PERIOD: f32 = 0.5;

/// Frames counted since the period began, the last figure, and the node
/// showing it with the figure and the screen size it was laid out for.
#[derive(Default)]
pub(crate) struct Fps {
    frames: u32,
    since: Option<f32>,
    rate: u32,
    shown: Option<(NodeId, u32, Vec2)>,
}

pub(crate) fn update(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::F10) {
        w.settings.fps = !w.settings.fps;
        w.settings.save();
    }
    let now = w.e.time.real.elapsed_secs();
    let screen = w.screen.size;
    let f = &mut w.fps;
    f.frames += 1;
    let since = *f.since.get_or_insert(now);
    if now - since >= PERIOD {
        f.rate = (f.frames as f32 / (now - since)).round() as u32;
        (f.frames, f.since) = (0, Some(now));
    }
    if let Some((id, rate, size)) = f.shown
        && (!w.settings.fps || rate != f.rate || size != screen || !w.ui.contains(id))
    {
        w.ui.remove(id);
        f.shown = None;
    }
    if !w.settings.fps || f.shown.is_some() {
        return;
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
    let k = screen.y / ROWS;
    let (glyphs, width) = font.layout(&format!("{} FPS", f.rate));
    let left = (screen.x - width * k) / 2.0;
    for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 1.0), (None, 0.0)] {
        for (x, image) in &glyphs {
            let mut glyph = UiNode::image(image.clone()).layout(Node {
                left: px(left + (x + d) * k),
                top: px((TOP + d) * k),
                width: px(font.size.x * k),
                height: px(font.size.y * k),
                ..default()
            });
            if let Some(c) = shade {
                glyph = glyph.tint(c);
            }
            ui.child(root, glyph);
        }
    }
    f.shown = Some((root, f.rate, screen));
}
