//! On-screen buttons that press a key, for players without a keyboard (on a
//! touchscreen, say): "Back" (Esc) on the level list and the code screen,
//! "Press Return when finished" (Return) on the code screen, "Restart" or
//! "Menu" (Esc) in a level, and the answers to the restart question (Y, N).
//! Pressing one sends that key's input, so the key's own handling serves
//! both (ours).

use l3d_engine::input::ButtonState;
use l3d_engine::input::{Key, KeyboardInput, NamedKey, NativeKey};
use l3d_engine::prelude::*;
use l3d_engine::window::PrimaryWindow;

use crate::lemming_cam::LemmingCam;
use crate::menu::AppState;
use crate::screen::Screen;
use crate::title::{Art, Glyphs, at, image_at};
use crate::ui::{Button, ImageNode, Interaction, Node, px};

/// The colour of a button's text under the pointer (as the level list's).
const HOVER: Color = Color::srgb(1.0, 0.9, 0.2);
/// The level button's centre and bottom, original pixels: under the panel's
/// right-hand icons, below the umbrella (ours).
const LEVEL_BUTTON_CENTRE: f32 = 284.0;
const LEVEL_BUTTON_BOTTOM: f32 = 197.0;
/// Where it goes when the skill row reaches under the icons: its distance
/// from the top of the screen, original pixels.
const LEVEL_BUTTON_TOP: f32 = 3.0;

pub struct KeyButtonsPlugin;

impl Plugin for KeyButtonsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, press.after(crate::ui::UiSystems::Focus))
            .add_systems(Update, (hover, level_button));
    }
}

/// A button that presses `key`; its text (its children) is drawn in
/// `colour`, or in the hover colour under the pointer.
#[derive(Component, Clone, Copy)]
pub struct KeyButton {
    pub key: KeyCode,
    pub colour: Color,
}

/// Sends a pressed button's key as keyboard input, released the next frame.
/// The key's state changes at once too, as a real key's does: Bevy takes in
/// the input only the next frame, when the screen may already have changed
/// on the input itself (the code screen's Esc then exited the title screen).
/// Taking it in again then changes nothing.
fn press(
    buttons: Query<(&Interaction, &KeyButton), Changed<Interaction>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut input: MessageWriter<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut held: Local<Vec<KeyCode>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let mut send = |key: KeyCode, state: ButtonState| {
        input.write(KeyboardInput {
            key_code: Some(key),
            logical_key: logical(key),
            state,
            text: None,
            repeat: false,
            window,
        });
    };
    for key in held.drain(..) {
        send(key, ButtonState::Released);
        keys.release(key);
    }
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            send(b.key, ButtonState::Pressed);
            keys.press(b.key);
            held.push(b.key);
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

fn hover(
    buttons: Query<(&Interaction, &KeyButton, Option<&Children>), Changed<Interaction>>,
    mut glyphs: Query<&mut ImageNode>,
) {
    for (i, b, children) in &buttons {
        let colour = if *i == Interaction::None {
            b.colour
        } else {
            HOVER
        };
        for &c in children.into_iter().flatten() {
            if let Ok(mut g) = glyphs.get_mut(c) {
                g.color = colour;
            }
        }
    }
}

/// A text button on a canvas screen (see `title`), with its left edge at
/// `x` and its top at `y` (canvas pixels).
#[allow(clippy::too_many_arguments)] // the text's place, look and key
pub(crate) fn spawn_text_button(
    commands: &mut Commands,
    parent: Entity,
    font: &Glyphs,
    text: &str,
    x: f32,
    y: f32,
    colour: Color,
    key: KeyCode,
) {
    let (glyphs, width) = font.layout(text);
    let button = commands
        .spawn((
            Button,
            KeyButton { key, colour },
            at(x, y, width, font.size.y),
            Node::default(),
        ))
        .id();
    for (gx, image) in glyphs {
        let (node, a, n) = image_at(image, at(gx, 0.0, font.size.x, font.size.y));
        let g = commands.spawn((node.with_color(colour), a, n)).id();
        commands.entity(button).add_child(g);
    }
    commands.entity(parent).add_child(button);
}

/// The button in a level, at the bottom right under the panel's icons, or
/// centred at the top of the screen when the window is too narrow for it to
/// clear the skill row there (the panel keeps the row to the left and the
/// icons to the right): "Restart" (Esc restarts the level, or asks to in Enhanced mode), or
/// "Menu" during a replay (Esc leaves for the level list). Hidden in the lemming view
/// (where Esc leaves the view; the panel's arrow does that) and while the
/// restart question is up.
#[derive(Component)]
struct LevelButton;

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn level_button(
    mut commands: Commands,
    art: Option<Res<Art>>,
    state: Res<State<AppState>>,
    game: Res<crate::Game>,
    cam: Res<LemmingCam>,
    confirm: Res<crate::confirm::RestartConfirm>,
    screen: Res<Screen>,
    buttons: Query<Entity, With<LevelButton>>,
    mut shown: Local<Option<(&'static str, Vec2)>>,
) {
    let label = (*state.get() == AppState::Playing
        && *cam == LemmingCam::Off
        && confirm.0.is_none())
    .then(|| {
        if game.replay.is_some() {
            "Menu"
        } else {
            "Restart"
        }
    });
    let wanted = label.map(|l| (l, screen.size));
    if *shown == wanted {
        return;
    }
    for e in &buttons {
        commands.entity(e).despawn();
    }
    *shown = None;
    let (Some((text, size)), Some(font)) = (wanted, art.as_ref().and_then(|a| a.large.as_ref()))
    else {
        return;
    };
    *shown = wanted;
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
    let button = commands
        .spawn((
            LevelButton,
            Button,
            KeyButton {
                key: KeyCode::Escape,
                colour,
            },
            Node {
                left: px(left.round()),
                top: px(top.round()),
                width: px(width * s),
                height: px(font.size.y * s),
                ..default()
            },
        ))
        .id();
    for (gx, image) in glyphs {
        let g = commands
            .spawn((
                ImageNode::new(image).with_color(colour),
                Node {
                    left: px(gx * s),
                    width: px(font.size.x * s),
                    height: px(font.size.y * s),
                    ..default()
                },
            ))
            .id();
        commands.entity(button).add_child(g);
    }
}
