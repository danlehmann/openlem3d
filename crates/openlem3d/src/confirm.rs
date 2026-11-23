//! The Enhanced mode's menu when Esc is pressed in a level (ours; the
//! original restarts at once): "Restart / Cancel / Exit" over the paused
//! level. R or Y restarts (as a replay), X leaves for the level list, N or
//! Esc carries on. The answers can be tapped, clicked or picked with a
//! gamepad's frame too (see `key_buttons`).

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
const TITLE: &str = "Paused";
/// The choices, left to right, and the keys they press: Cancel in the
/// middle, where a gamepad's frame starts.
const CHOICES: [(&str, KeyCode); 3] = [
    ("Restart", KeyCode::KeyR),
    ("Cancel", KeyCode::KeyN),
    ("Exit", KeyCode::KeyX),
];
const TOP: f32 = 180.0;
/// Space between the widest choice and the next, in the canvas's pixels.
const GAP: f32 = 48.0;

/// The open menu's overlay, which a gamepad picks within.
pub(crate) fn root(w: &World) -> Option<NodeId> {
    w.confirm.root
}

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
    // Text with a dark shadow, its left edge at `left`.
    let write = |ui: &mut crate::ui::Ui, text: &str, left: f32, y: f32| {
        let (glyphs, _) = font.layout(text);
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
    };
    let (_, title_width) = font.layout(TITLE);
    write(ui, TITLE, (SCREEN.x - title_width * SCALE) / 2.0, TOP);
    // The choices in a row of equal columns, each word in the middle of its
    // own (so Cancel lies under the title), and a button round it.
    let widths: Vec<f32> = CHOICES
        .iter()
        .map(|(t, _)| font.layout(t).1 * SCALE)
        .collect();
    let column = widths.iter().cloned().fold(0.0, f32::max) + GAP;
    let y = TOP + (font.size.y + 8.0) * SCALE;
    let pad = font.size.y * SCALE / 2.0;
    let first = SCREEN.x / 2.0 - column * (CHOICES.len() - 1) as f32 / 2.0;
    for (i, ((text, key), width)) in CHOICES.iter().zip(widths).enumerate() {
        let left = first + column * i as f32 - width / 2.0;
        write(ui, text, left, y);
        let b = ui.child(
            canvas,
            UiNode::new().button().at(
                left - pad,
                y - pad,
                width + 2.0 * pad,
                font.size.y * SCALE + 2.0 * pad,
            ),
        );
        w.key_buttons.add(
            b,
            KeyButton {
                key: *key,
                colour: Color::WHITE,
            },
        );
    }
}
