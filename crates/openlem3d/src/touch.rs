//! Touch controls: dragging moves the camera. The panel's buttons (camera,
//! pause, turn, release rate) cover the keyboard-only functions, so the game
//! is fully playable without a keyboard. Taps on lemmings are handled by the
//! HUD.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::ViewCamera;
use crate::menu::AppState;
use crate::ui::Interaction;

/// Finger travel (logical pixels) below which a touch counts as a tap.
pub const TAP_SLOP: f32 = 12.0;

/// Camera turn per pixel of one-finger (or, in Enhanced mode, action-button)
/// horizontal drag (radians).
pub const TURN_PER_PX: f32 = 0.006;
/// Camera travel per pixel of drag (grid units).
pub const MOVE_PER_PX: f32 = 0.02;
/// How far two fingers twist (radians) before the world turns with them, so
/// that dragging and pinching don't turn it by the way (ours).
const TWIST_START: f32 = 0.2;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchStarts>()
            .add_systems(
                PreUpdate,
                record_touch_starts.after(crate::ui::UiSystems::Focus),
            )
            .add_systems(
                Update,
                touch_camera
                    .before(crate::camera_controls)
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                PreUpdate,
                touch_over_mouse
                    .after(bevy::input::InputSystems)
                    .before(crate::ui::UiSystems::Focus),
            );
    }
}

/// While a finger is down, the window forgets the mouse pointer's last
/// position, so that buttons are pressed where the finger is: Bevy's UI
/// uses the pointer's position when it has one, and the touch's only
/// otherwise (taps on skills did nothing while the pointer had last been
/// over the window). The real pointer does not move.
fn touch_over_mouse(touches: Res<Touches>, mut windows: Query<&mut Window>) {
    if touches.iter().next().is_none() && !touches.any_just_released() {
        return;
    }
    for mut window in &mut windows {
        if window.physical_cursor_position().is_some() {
            window.set_physical_cursor_position(None);
        }
    }
}

/// The screen each finger now down (or just lifted) landed on, and whether
/// it landed on a button.
#[derive(Resource, Default)]
pub struct TouchStarts(HashMap<u64, (AppState, bool)>);

impl TouchStarts {
    /// Whether the finger landed on a button. Its button is no longer pressed
    /// when it lifts, but lifting it is not a tap on what lies under it.
    pub fn on_button(&self, id: u64) -> bool {
        self.0.get(&id).is_some_and(|&(_, button)| button)
    }
}

fn record_touch_starts(
    touches: Res<Touches>,
    state: Res<State<AppState>>,
    ui: Query<&Interaction>,
    mut starts: ResMut<TouchStarts>,
) {
    starts
        .0
        .retain(|&id, _| touches.get_pressed(id).is_some() || touches.just_released(id));
    let button = ui.iter().any(|i| *i == Interaction::Pressed);
    for t in touches.iter_just_pressed() {
        starts.0.insert(t.id(), (*state.get(), button));
    }
}

/// Fingers lifting on the screen they landed on. A finger that chose
/// something on one screen (landing on a level in the practice grid, say)
/// is then not also a tap on the next, which would skip it.
#[derive(SystemParam)]
pub struct Lifted<'w> {
    touches: Res<'w, Touches>,
    starts: Res<'w, TouchStarts>,
    state: Res<'w, State<AppState>>,
}

impl Lifted<'_> {
    /// Whether a finger that landed on this screen lifted.
    pub fn any(&self) -> bool {
        self.touches
            .iter_just_released()
            .any(|t| self.starts.0.get(&t.id()).map(|s| &s.0) == Some(self.state.get()))
    }
}

/// One finger: horizontal drag turns, vertical drag moves forward/back.
/// Two fingers: as [`two_fingers`].
fn touch_camera(
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    (screen, camera, game): (
        Res<crate::screen::Screen>,
        Res<crate::scene_render::SceneCamera>,
        Res<crate::Game>,
    ),
    mut twist: Local<Twist>,
    mut cams: Query<&mut ViewCamera>,
) {
    let active: Vec<_> = touches.iter().collect();
    if active.len() < 2 {
        *twist = Twist::default();
    }
    if ui.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(first) = active.first() else { return };
    // Ignore fingers that have not left the tap radius: they may be taps.
    if active.len() == 1 && first.distance().length() < TAP_SLOP {
        return;
    }
    let delta = active.iter().map(|t| t.delta()).sum::<Vec2>() / active.len() as f32;
    if let [a, b, ..] = active[..] {
        let (now, before) = (
            (a.position(), b.position()),
            (a.previous_position(), b.previous_position()),
        );
        let fingers = TwoFingers {
            delta,
            spread: now.0.distance(now.1) - before.0.distance(before.1),
            twist: twist.turn((before.1 - before.0).angle_to(now.1 - now.0)),
            turning: false,
            toward: crate::screen_ray(&screen, &camera, screen.on_screen((now.0 + now.1) / 2.0)),
        };
        for mut cam in &mut cams {
            two_fingers(&mut cam, &fingers, &game);
        }
        return;
    }
    for mut cam in &mut cams {
        let fwd = cam.forward();
        cam.yaw += delta.x * TURN_PER_PX;
        cam.pos += fwd * delta.y * MOVE_PER_PX;
    }
}

/// Two fingers' move since the last frame, on a touchscreen or trackpad
/// (pixels and radians).
pub(crate) struct TwoFingers {
    /// How far they moved together.
    pub delta: Vec2,
    /// How much further apart they are.
    pub spread: f32,
    /// How far they turned round each other, clockwise on the screen (see
    /// [`Twist`]).
    pub twist: f32,
    /// Whether dragging turns the camera rather than moving it sideways.
    pub turning: bool,
    /// The way from the camera through where they are (or the pointer).
    pub toward: Option<Vec3>,
}

/// Two fingers: dragging moves the camera sideways and up and down (or,
/// `turning`, turns it and moves it forward and back, as one finger does);
/// spreading them apart moves it towards where they are, pinching them
/// together back; twisting them turns the world about the spot there, as if
/// turning it with them.
pub(crate) fn two_fingers(cam: &mut ViewCamera, f: &TwoFingers, game: &crate::Game) {
    let fwd = cam.forward();
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    if f.turning {
        cam.yaw += f.delta.x * TURN_PER_PX;
        cam.pos += fwd * f.delta.y * MOVE_PER_PX;
    } else {
        cam.pos += (-right * f.delta.x + Vec3::Y * f.delta.y) * MOVE_PER_PX;
    }
    cam.pos += f.toward.unwrap_or(fwd) * f.spread * MOVE_PER_PX;
    if f.twist != 0.0
        && let Some(spot) = crate::spot_under(game, cam.pos, f.toward)
    {
        cam.orbit(spot, f.twist);
    }
    cam.glide = None;
}

/// A two-finger twist so far: it turns the world only once it has gone past
/// [`TWIST_START`].
#[derive(Default)]
pub(crate) struct Twist {
    total: f32,
    on: bool,
}

impl Twist {
    /// How far a twist by `angle` turns the world.
    pub fn turn(&mut self, angle: f32) -> f32 {
        self.total += angle;
        self.on |= self.total.abs() > TWIST_START;
        if self.on { angle } else { 0.0 }
    }
}
