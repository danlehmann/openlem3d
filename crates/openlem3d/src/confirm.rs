//! The Enhanced mode's question before Esc restarts a level as a replay
//! (ours; the original restarts at once): "Restart level? Y/N" over the
//! paused level. Y or Return restarts; N or Esc carries on. The answers can
//! be tapped or clicked too (see `key_buttons`).

use l3d_engine::prelude::*;

use crate::World;
use crate::key_buttons::KeyButton;
use crate::ui::{Node, NodeId, UiNode, percent};

/// The open question, holding whether the level was paused before it, and
/// its overlay.
#[derive(Default)]
pub(crate) struct Confirm {
    pub(crate) question: Option<bool>,
    root: Option<NodeId>,
}

/// The overlay's canvas: the briefing's, shared with the mode notice.
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const SCALE: f32 = 3.0;
const LINES: [&str; 2] = ["Restart level?", "Y / N"];
const TOP: f32 = 180.0;

/// Leaving the level closes the question.
pub(crate) fn close(w: &mut World) {
    w.confirm.question = None;
    w.ui.remove_opt(&mut w.confirm.root);
}

/// Builds the overlay when the question opens and removes it when it closes.
pub(crate) fn show(w: &mut World) {
    if w.confirm.question.is_none() {
        w.ui.remove_opt(&mut w.confirm.root);
        return;
    }
    if w.confirm.root.is_some() {
        return;
    }
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let ui = &mut w.ui;
    let root = ui.root(
        UiNode::new()
            .layout(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .background(Color::srgba(0.0, 0.0, 0.0, 0.4)),
    );
    w.confirm.root = Some(root);
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    for (row, line) in LINES.iter().enumerate() {
        let (glyphs, width) = font.layout(line);
        let left = (SCREEN.x - width * SCALE) / 2.0;
        let y = TOP + row as f32 * (font.size.y + 4.0) * SCALE;
        // A dark shadow, then the text.
        for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
            for (x, image) in &glyphs {
                let mut glyph = UiNode::image(image.clone()).at(
                    left + x * SCALE + d,
                    y + d,
                    font.size.x * SCALE,
                    font.size.y * SCALE,
                );
                if let Some(c) = shade {
                    glyph = glyph.tint(c);
                }
                ui.child(canvas, glyph);
            }
        }
    }
    // The answers as buttons, each reaching to the slash between them.
    let (glyphs, width) = font.layout(LINES[1]);
    let Some(&(slash, _)) = glyphs.get(1) else {
        return;
    };
    let left = (SCREEN.x - width * SCALE) / 2.0;
    let y = TOP + (font.size.y + 4.0) * SCALE;
    let mid = left + (slash + font.size.x / 2.0) * SCALE;
    let pad = font.size.y * SCALE;
    let right = left + width * SCALE + pad;
    for (x0, x1, key) in [
        (left - pad, mid, KeyCode::KeyY),
        (mid, right, KeyCode::KeyN),
    ] {
        let b = ui.child(
            canvas,
            UiNode::new()
                .button()
                .at(x0, y - pad / 2.0, x1 - x0, font.size.y * SCALE + pad),
        );
        w.key_buttons.add(
            b,
            KeyButton {
                key,
                colour: Color::WHITE,
            },
        );
    }
}
