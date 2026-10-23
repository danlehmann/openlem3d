//! On-screen buttons that press a key, for players without a keyboard (on a
//! touchscreen, say): "Back" (Esc) on the level list and the code screen,
//! "Press Return when finished" (Return) on the code screen, "Restart" or
//! "Menu" (Esc) in a level, and the answers to the restart question (Y, N).
//! Pressing one sends that key's input, so the key's own handling serves
//! both (ours).

use l3d_engine::input::{ButtonState, Key, KeyboardInput, NamedKey, NativeKey};
use l3d_engine::prelude::*;

use crate::World;
use crate::lemming_cam::LemmingCam;
use crate::menu::AppState;
use crate::title::Glyphs;
use crate::ui::{Interaction, Node, NodeId, Ui, UiNode, px};

/// The colour of a button's text under the pointer (as the level list's).
const HOVER: Color = Color::srgb(1.0, 0.9, 0.2);
/// The level button's centre and bottom, original pixels: under the panel's
/// right-hand icons, below the umbrella (ours).
const LEVEL_BUTTON_CENTRE: f32 = 284.0;
const LEVEL_BUTTON_BOTTOM: f32 = 197.0;
/// Where it goes when the skill row reaches under the icons: its distance
/// from the top of the screen, original pixels.
const LEVEL_BUTTON_TOP: f32 = 3.0;

/// What a button presses: `key`; its text (its children) is drawn in
/// `colour`, or in the hover colour under the pointer.
#[derive(Clone, Copy)]
pub struct KeyButton {
    pub key: KeyCode,
    pub colour: Color,
}

/// The key buttons there are, and the keys pressed for one frame.
#[derive(Default)]
pub(crate) struct KeyButtons {
    buttons: Vec<(NodeId, KeyButton)>,
    held: Vec<KeyCode>,
    /// The level button, with its text and the screen size it was made for.
    level: Option<(NodeId, &'static str, Vec2)>,
}

impl KeyButtons {
    /// Makes node `id` (a button) press `button.key`.
    pub(crate) fn add(&mut self, id: NodeId, button: KeyButton) {
        self.buttons.push((id, button));
    }
}

/// Sends a pressed button's key as keyboard input, released the next frame.
/// The key's state changes at once too, as a real key's does.
pub(crate) fn press(w: &mut World) {
    let ui = &w.ui;
    let kb = &mut w.key_buttons;
    kb.buttons.retain(|(id, _)| ui.contains(*id));
    let input = &mut w.e.input;
    let mut send = |key: KeyCode, state: ButtonState| {
        input.keyboard.push(KeyboardInput {
            key_code: Some(key),
            logical_key: logical(key),
            state,
            text: None,
            repeat: false,
        });
        match state {
            ButtonState::Pressed => input.keys.press(key),
            ButtonState::Released => input.keys.release(key),
        }
    };
    for key in kb.held.drain(..) {
        send(key, ButtonState::Released);
    }
    for (id, b) in &kb.buttons {
        if ui.just_pressed(*id) {
            send(b.key, ButtonState::Pressed);
            kb.held.push(b.key);
        }
    }
}

/// The key a [`KeyButton`]'s key produces.
fn logical(key: KeyCode) -> Key {
    match key {
        KeyCode::Escape => Key::Named(NamedKey::Escape),
        KeyCode::Enter => Key::Named(NamedKey::Enter),
        KeyCode::KeyY => Key::Character("y".into()),
        KeyCode::KeyN => Key::Character("n".into()),
        _ => Key::Unidentified(NativeKey::Unidentified),
    }
}

pub(crate) fn hover(w: &mut World) {
    let ui = &mut w.ui;
    for (id, b) in &w.key_buttons.buttons {
        if !ui.changed(*id) {
            continue;
        }
        let colour = if ui.interaction(*id) == Interaction::None {
            b.colour
        } else {
            HOVER
        };
        for c in ui.children(*id).to_vec() {
            if let Some(g) = ui.get_mut(c) {
                g.set_tint(colour);
            }
        }
    }
}

/// A text button on a canvas screen (see `title`), with its left edge at
/// `x` and its top at `y` (canvas pixels).
#[allow(clippy::too_many_arguments)] // the text's place, look and key
pub(crate) fn spawn_text_button(
    ui: &mut Ui,
    buttons: &mut KeyButtons,
    parent: NodeId,
    font: &Glyphs,
    text: &str,
    x: f32,
    y: f32,
    colour: Color,
    key: KeyCode,
) {
    let (glyphs, width) = font.layout(text);
    let button = ui.child(parent, UiNode::new().button().at(x, y, width, font.size.y));
    buttons.add(button, KeyButton { key, colour });
    for (gx, image) in glyphs {
        ui.child(
            button,
            UiNode::image(image)
                .tint(colour)
                .at(gx, 0.0, font.size.x, font.size.y),
        );
    }
}

/// The button in a level, at the bottom right under the panel's icons, or
/// centred at the top of the screen when the window is too narrow for it to
/// clear the skill row there (the panel keeps the row to the left and the
/// icons to the right): "Restart" (Esc restarts the level, or asks to in Enhanced mode), or
/// "Menu" during a replay (Esc leaves for the level list). Hidden in the lemming view
/// (where Esc leaves the view; the panel's arrow does that) and while the
/// restart question is up.
pub(crate) fn level_button(w: &mut World) {
    let label = (w.state == AppState::Playing
        && w.lemming_cam == LemmingCam::Off
        && w.confirm.question.is_none())
    .then(|| {
        if w.game.replay.is_some() {
            "Menu"
        } else {
            "Restart"
        }
    });
    let size = w.screen.size;
    let shown = w.key_buttons.level.map(|(_, t, s)| (t, s));
    let wanted = label.map(|l| (l, size));
    if shown == wanted {
        return;
    }
    if let Some((id, ..)) = w.key_buttons.level.take() {
        w.ui.remove(id);
    }
    let (Some((text, size)), Some(font)) = (wanted, w.art.as_ref().and_then(|a| a.large.as_ref()))
    else {
        return;
    };
    // Original pixels, scaled as the panel is.
    let s = (size.y / 200.0).max(1.0);
    let (glyphs, width) = font.layout(text);
    let colour = Color::WHITE;
    let skills_end = crate::panel::SKILL_BUTTONS[8].0 + 32.0;
    let right = size.x - (320.0 - LEVEL_BUTTON_CENTRE) * s - width * s / 2.0;
    let (left, top) = if right >= skills_end * s {
        (
            right,
            size.y - (200.0 - LEVEL_BUTTON_BOTTOM + font.size.y) * s,
        )
    } else {
        ((size.x - width * s) / 2.0, LEVEL_BUTTON_TOP * s)
    };
    let ui = &mut w.ui;
    let button = ui.root(UiNode::new().button().layout(Node {
        left: px(left.round()),
        top: px(top.round()),
        width: px(width * s),
        height: px(font.size.y * s),
        ..default()
    }));
    w.key_buttons.add(
        button,
        KeyButton {
            key: KeyCode::Escape,
            colour,
        },
    );
    w.key_buttons.level = Some((button, text, size));
    for (gx, image) in glyphs {
        ui.child(
            button,
            UiNode::image(image).tint(colour).layout(Node {
                left: px(gx * s),
                width: px(font.size.x * s),
                height: px(font.size.y * s),
                ..default()
            }),
        );
    }
}
