//! The Enhanced mode's question before Esc restarts a level as a replay
//! (ours; the original restarts at once): "Restart level? Y/N" over the
//! paused level. Y or Return restarts; N or Esc carries on. The answers can
//! be tapped or clicked too (see `key_buttons`).

use bevy::prelude::*;

use crate::key_buttons::KeyButton;
use crate::menu::AppState;
use crate::title::{Art, At, Canvas, at, image_at};
use crate::ui::{BackgroundColor, Button, ImageNode, Node, percent};

pub struct ConfirmPlugin;

impl Plugin for ConfirmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RestartConfirm>()
            .add_systems(Update, show.run_if(in_state(AppState::Playing)))
            .add_systems(OnExit(AppState::Playing), close);
    }
}

/// The open question, holding whether the level was paused before it.
#[derive(Resource, Default)]
pub(crate) struct RestartConfirm(pub(crate) Option<bool>);

/// The question's overlay.
#[derive(Component)]
struct ConfirmRoot;

/// The overlay's canvas: the briefing's, shared with the mode notice.
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const SCALE: f32 = 3.0;
const LINES: [&str; 2] = ["Restart level?", "Y / N"];
const TOP: f32 = 180.0;

fn close(
    mut commands: Commands,
    mut confirm: ResMut<RestartConfirm>,
    roots: Query<Entity, With<ConfirmRoot>>,
) {
    confirm.0 = None;
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Builds the overlay when the question opens and removes it when it closes.
fn show(
    mut commands: Commands,
    confirm: Res<RestartConfirm>,
    art: Option<Res<Art>>,
    roots: Query<Entity, With<ConfirmRoot>>,
) {
    if confirm.0.is_none() {
        for e in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let root = commands
        .spawn((
            ConfirmRoot,
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
        ))
        .id();
    let canvas = commands.spawn((Canvas(SCREEN), Node::default())).id();
    commands.entity(root).add_child(canvas);
    for (row, line) in LINES.iter().enumerate() {
        let (glyphs, width) = font.layout(line);
        let left = (SCREEN.x - width * SCALE) / 2.0;
        let y = TOP + row as f32 * (font.size.y + 4.0) * SCALE;
        // A dark shadow, then the text.
        for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
            for (x, image) in &glyphs {
                let mut bundle: (ImageNode, At, Node) = image_at(
                    image.clone(),
                    at(
                        left + x * SCALE + d,
                        y + d,
                        font.size.x * SCALE,
                        font.size.y * SCALE,
                    ),
                );
                if let Some(c) = shade {
                    bundle.0.color = c;
                }
                let g = commands.spawn(bundle).id();
                commands.entity(canvas).add_child(g);
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
        let b = commands
            .spawn((
                Button,
                KeyButton {
                    key,
                    colour: Color::WHITE,
                },
                at(x0, y - pad / 2.0, x1 - x0, font.size.y * SCALE + pad),
                Node::default(),
            ))
            .id();
        commands.entity(canvas).add_child(b);
    }
}
