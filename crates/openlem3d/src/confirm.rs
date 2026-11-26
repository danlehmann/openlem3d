//! The Enhanced mode's menu when Esc is pressed in a level (ours; the
//! original restarts at once): "Restart / Exit / Options / Cancel" over the
//! paused level. R or Y restarts (as a replay), F12 opens the options, X
//! leaves for the level list, N or Esc carries on. The answers can be
//! tapped, clicked or picked with a gamepad's frame too (see `key_buttons`).
//! In VR it is a popup: only its card shows, floating ahead.

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
    /// The card behind the menu, shown as a popup (VR), and Cancel, where a
    /// gamepad starts.
    card: Option<NodeId>,
    cancel: Option<NodeId>,
    /// The menu's area on its canvas: what a popup shows.
    area: Rect,
    /// Shown as a popup: its card, no darkening of the screen (set by VR).
    pub(crate) popup: bool,
}

/// The overlay's canvas: the briefing's, shared with the mode notice.
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const SCALE: f32 = 3.0;
const TITLE: &str = "Paused";
/// The choices, left to right, and the keys they press (owner's order); a
/// gamepad's frame starts on Cancel.
const CHOICES: [(&str, KeyCode); 4] = [
    ("Restart", KeyCode::KeyR),
    ("Exit", KeyCode::KeyX),
    ("Options", KeyCode::F12),
    ("Cancel", KeyCode::KeyN),
];
/// The popup's card: its colour, and its margin round the menu.
const CARD: Color = Color::srgba(0.06, 0.07, 0.12, 0.92);
const CARD_MARGIN: f32 = 20.0;
const TOP: f32 = 180.0;
/// Space between the widest choice and the next, in the canvas's pixels.
const GAP: f32 = 48.0;

/// The open menu's overlay, which a gamepad picks within.
pub(crate) fn root(w: &World) -> Option<NodeId> {
    w.confirm.root
}

/// Cancel, where a gamepad starts.
pub(crate) fn first_choice(w: &World) -> Option<NodeId> {
    w.confirm.cancel
}

/// The popup's part of the screen, in physical pixels: left, top, width,
/// height.
#[cfg_attr(not(feature = "xr"), allow(dead_code))]
pub(crate) fn popup_pixels(confirm: &Confirm, screen: &crate::screen::Screen) -> UVec4 {
    let s = (screen.size.x / SCREEN.x).min(screen.size.y / SCREEN.y);
    let origin = (screen.size - SCREEN * s) / 2.0;
    let physical = screen.pixels.z as f32 / screen.size.x.max(1.0);
    let a = confirm.area;
    let min = ((origin + a.min * s) * physical).round().max(Vec2::ZERO);
    let size = (a.size() * s * physical).round().max(Vec2::ONE);
    UVec4::new(
        screen.pixels.x + min.x as u32,
        screen.pixels.y + min.y as u32,
        size.x as u32,
        size.y as u32,
    )
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
    if let Some(root) = w.confirm.root {
        // A popup has its card and leaves the screen undarkened.
        let popup = w.confirm.popup;
        if let Some(card) = w.confirm.card {
            w.ui.set_visible(card, popup);
        }
        if let Some(n) = w.ui.get_mut(root) {
            n.background = Some(Color::srgba(0.0, 0.0, 0.0, if popup { 0.0 } else { 0.4 }));
        }
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
    let card = ui.child(canvas, UiNode::new().background(CARD).hidden());
    w.confirm.card = Some(card);
    // Text with a dark shadow, its left edge at `left`.
    let write = |ui: &mut crate::ui::Ui, text: &str, left: f32, y: f32, scale: f32| {
        let (glyphs, _) = font.layout(text);
        for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
            for (x, image) in &glyphs {
                let mut glyph = UiNode::image(image.clone()).at(
                    left + x * scale + d,
                    y + d,
                    font.size.x * scale,
                    font.size.y * scale,
                );
                if let Some(c) = shade {
                    glyph = glyph.tint(c);
                }
                ui.child(canvas, glyph);
            }
        }
    };
    let (_, title_width) = font.layout(TITLE);
    write(
        ui,
        TITLE,
        (SCREEN.x - title_width * SCALE) / 2.0,
        TOP,
        SCALE,
    );
    // The choices in a row of equal columns, each word in the middle of its
    // own, and a button round it; smaller than the title where the row
    // would not fit at its size.
    let widest = CHOICES
        .iter()
        .map(|(t, _)| font.layout(t).1)
        .fold(0.0, f32::max);
    let n = CHOICES.len() as f32;
    let scale =
        SCALE.min((SCREEN.x - 2.0 * CARD_MARGIN - GAP * (n - 1.0)) / (n * widest + font.size.y));
    let column = widest * scale + GAP;
    let y = TOP + (font.size.y + 8.0) * SCALE;
    let pad = font.size.y * scale / 2.0;
    let first = SCREEN.x / 2.0 - column * (n - 1.0) / 2.0;
    let half = column * (n - 1.0) / 2.0 + widest * scale / 2.0 + pad;
    let area = Rect::new(
        SCREEN.x / 2.0 - half - CARD_MARGIN,
        TOP - CARD_MARGIN,
        SCREEN.x / 2.0 + half + CARD_MARGIN,
        y + font.size.y * scale + pad + CARD_MARGIN,
    );
    w.confirm.area = area;
    if let Some(n) = ui.get_mut(card) {
        n.at = Some(area);
    }
    for (i, (text, key)) in CHOICES.iter().enumerate() {
        let width = font.layout(text).1 * scale;
        let left = first + column * i as f32 - width / 2.0;
        write(ui, text, left, y, scale);
        let b = ui.child(
            canvas,
            UiNode::new().button().at(
                left - pad,
                y - pad,
                width + 2.0 * pad,
                font.size.y * scale + 2.0 * pad,
            ),
        );
        if *key == KeyCode::KeyN {
            w.confirm.cancel = Some(b);
        }
        w.key_buttons.add(
            b,
            KeyButton {
                key: *key,
                colour: Color::WHITE,
            },
        );
    }
}
