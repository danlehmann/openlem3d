//! Touch controls: dragging moves the camera, and on-screen buttons replace
//! the keyboard-only functions (camera presets, pause), so the game is fully
//! playable without a keyboard. Taps on lemmings are handled by the HUD.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::menu::AppState;
use crate::{Game, LevelInfo, ViewCamera};

/// Finger travel (logical pixels) below which a touch counts as a tap.
pub const TAP_SLOP: f32 = 12.0;

/// Camera turn per pixel of one-finger horizontal drag (radians).
const TURN_PER_PX: f32 = 0.006;
/// Camera travel per pixel of drag (grid units).
const MOVE_PER_PX: f32 = 0.02;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_buttons)
            .add_systems(Update, (touch_camera, touch_buttons, show_in_play).run_if(in_state(AppState::Playing)))
            .add_systems(OnEnter(AppState::Menu), hide);
    }
}

#[derive(Component)]
enum TouchButton {
    Camera(usize),
    Pause,
}

#[derive(Component)]
struct TouchBar;

fn spawn_buttons(mut commands: Commands) {
    commands
        .spawn((
            TouchBar,
            Node {
                position_type: PositionType::Absolute,
                top: px(8),
                right: px(8),
                column_gap: px(4),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            let labels = [("1", TouchButton::Camera(0)), ("2", TouchButton::Camera(1)), ("3", TouchButton::Camera(2)), ("4", TouchButton::Camera(3)), ("II", TouchButton::Pause)];
            for (label, kind) in labels {
                bar.spawn((
                    Button,
                    kind,
                    Node { width: px(44), height: px(44), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.2, 0.75)),
                ))
                .with_child((Text::new(label), TextFont { font_size: FontSize::Px(18.0), ..default() }));
            }
        });
}

fn show_in_play(mut bars: Query<&mut Visibility, With<TouchBar>>) {
    for mut v in &mut bars {
        *v = Visibility::Inherited;
    }
}

fn hide(mut bars: Query<&mut Visibility, With<TouchBar>>) {
    for mut v in &mut bars {
        *v = Visibility::Hidden;
    }
}

fn touch_buttons(
    buttons: Query<(&Interaction, &TouchButton), Changed<Interaction>>,
    info: Option<Res<LevelInfo>>,
    mut game: ResMut<Game>,
    mut cams: Query<&mut ViewCamera>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            TouchButton::Camera(i) => {
                if let Some(info) = &info {
                    for mut cam in &mut cams {
                        cam.set_preset(&info.cameras[i]);
                    }
                }
            }
            TouchButton::Pause => game.paused = !game.paused,
        }
    }
}

/// One finger: horizontal drag turns, vertical drag moves forward/back.
/// Two fingers: drag strafes sideways and changes height.
fn touch_camera(touches: Res<Touches>, ui: Query<&Interaction>, mut cams: Query<&mut ViewCamera>) {
    if ui.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let active: Vec<_> = touches.iter().collect();
    let Some(first) = active.first() else { return };
    // Ignore fingers that have not left the tap radius: they may be taps.
    if active.len() == 1 && first.distance().length() < TAP_SLOP {
        return;
    }
    let delta = active.iter().map(|t| t.delta()).sum::<Vec2>() / active.len() as f32;
    for mut cam in &mut cams {
        let fwd = cam.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        if active.len() == 1 {
            cam.yaw += delta.x * TURN_PER_PX;
            cam.pos += fwd * delta.y * MOVE_PER_PX;
        } else {
            cam.pos += -right * delta.x * MOVE_PER_PX + Vec3::Y * delta.y * MOVE_PER_PX;
        }
    }
}
