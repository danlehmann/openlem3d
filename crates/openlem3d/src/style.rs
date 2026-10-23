//! The presentation mode: Original (the game as it was, its 3D view at
//! 320×200) or Enhanced (ours: full resolution, smooth camera moves, a
//! question before Esc restarts a level). A label at the top right of the
//! title screen shows it and switches it when clicked; Tab switches it on
//! the title and during a level, where the new mode is named briefly.

use l3d_engine::prelude::*;

use crate::World;
use crate::settings::Settings;
use crate::ui::{Node, NodeId, UiNode, percent};

/// The title canvas, in the original's pixels.
const TITLE_SCREEN: Vec2 = Vec2::new(320.0, 200.0);
/// The notice's canvas: the briefing's, shared with the preview overlay.
const NOTICE_SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const NOTICE_SCALE: f32 = 3.0;
const NOTICE_Y: f32 = 60.0;
/// How long the notice shows, in seconds.
const NOTICE_SECONDS: f32 = 1.5;

fn name(enhanced: bool) -> &'static str {
    if enhanced { "Enhanced" } else { "Original" }
}

fn switch(settings: &mut Settings) {
    settings.enhanced = !settings.enhanced;
    settings.save();
}

/// The title's mode label (a button whose children are the glyphs) with
/// the mode it shows, and the notice in a level with when it was switched.
#[derive(Default)]
pub(crate) struct Style {
    label: Option<(NodeId, bool)>,
    notice: Option<(NodeId, f32)>,
}

pub(crate) fn toggle_on_title(w: &mut World) {
    let clicked = w.style.label.is_some_and(|(id, _)| w.ui.just_pressed(id));
    if w.e.input.keys.just_pressed(KeyCode::Tab) || clicked {
        switch(&mut w.settings);
    }
}

/// Builds the label once the title exists, and again when the mode changes.
pub(crate) fn title_label(w: &mut World) {
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let Some(canvas) = w.title.canvas.filter(|&c| w.ui.contains(c)) else {
        return;
    };
    let enhanced = w.settings.enhanced;
    if let Some((id, shown)) = w.style.label {
        if w.ui.contains(id) && shown == enhanced {
            return;
        }
        w.ui.remove(id);
    }
    let (glyphs, width) = font.layout(&format!("Mode: {}", name(enhanced)));
    let left = TITLE_SCREEN.x - 2.0 - width;
    let ui = &mut w.ui;
    let label = ui.child(
        canvas,
        UiNode::new().button().at(left, 0.0, width, font.size.y),
    );
    for (x, image) in glyphs {
        ui.child(
            label,
            UiNode::image(image).at(x, 0.0, font.size.x, font.size.y),
        );
    }
    w.style.label = Some((label, enhanced));
}

/// Leaving the level takes the notice away.
pub(crate) fn close_notice(w: &mut World) {
    if let Some((id, _)) = w.style.notice.take() {
        w.ui.remove(id);
    }
}

pub(crate) fn toggle_in_level(w: &mut World) {
    let now = w.e.time.real.elapsed_secs();
    let changed = w.e.input.keys.just_pressed(KeyCode::Tab);
    if changed {
        switch(&mut w.settings);
    }
    if let Some((id, since)) = w.style.notice
        && (changed || now - since > NOTICE_SECONDS)
    {
        w.ui.remove(id);
        w.style.notice = None;
    }
    if !changed {
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
    w.style.notice = Some((root, now));
    let canvas = ui.child(root, UiNode::new().canvas(NOTICE_SCREEN));
    let (glyphs, width) = font.layout(name(w.settings.enhanced));
    let left = (NOTICE_SCREEN.x - width * NOTICE_SCALE) / 2.0;
    // A dark shadow, then the text.
    for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
        for (x, image) in &glyphs {
            let mut glyph = UiNode::image(image.clone()).at(
                left + x * NOTICE_SCALE + d,
                NOTICE_Y + d,
                font.size.x * NOTICE_SCALE,
                font.size.y * NOTICE_SCALE,
            );
            if let Some(c) = shade {
                glyph = glyph.tint(c);
            }
            ui.child(canvas, glyph);
        }
    }
}
