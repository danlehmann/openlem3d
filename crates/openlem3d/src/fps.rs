//! The frames-per-second counter (ours): F10 shows or hides it, on every
//! screen, and the setting is kept.

use l3d_engine::prelude::*;

use crate::World;
use crate::ui::{Node, NodeId, UiNode, percent};

/// The counter's canvas, and how far down it the text's top is (centred,
/// under the level's top line).
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const TOP: f32 = 40.0;
/// How long frames are counted for each figure, in seconds.
const PERIOD: f32 = 0.5;

/// Frames counted since the period began, the last figure, and the node
/// showing it with the figure it shows.
#[derive(Default)]
pub(crate) struct Fps {
    frames: u32,
    since: Option<f32>,
    rate: u32,
    shown: Option<(NodeId, u32)>,
}

pub(crate) fn update(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::F10) {
        w.settings.fps = !w.settings.fps;
        w.settings.save();
    }
    let now = w.e.time.real.elapsed_secs();
    let f = &mut w.fps;
    f.frames += 1;
    let since = *f.since.get_or_insert(now);
    if now - since >= PERIOD {
        f.rate = (f.frames as f32 / (now - since)).round() as u32;
        (f.frames, f.since) = (0, Some(now));
    }
    if let Some((id, rate)) = f.shown
        && (!w.settings.fps || rate != f.rate || !w.ui.contains(id))
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
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    let (glyphs, width) = font.layout(&format!("{} FPS", f.rate));
    for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 1.0), (None, 0.0)] {
        for (x, image) in &glyphs {
            let mut glyph = UiNode::image(image.clone()).at(
                (SCREEN.x - width) / 2.0 + x + d,
                TOP + d,
                font.size.x,
                font.size.y,
            );
            if let Some(c) = shade {
                glyph = glyph.tint(c);
            }
            ui.child(canvas, glyph);
        }
    }
    f.shown = Some((root, f.rate));
}
