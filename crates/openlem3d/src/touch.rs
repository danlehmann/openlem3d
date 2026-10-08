//! Touch controls: dragging moves the camera. The panel's buttons (camera,
//! pause, turn, release rate) cover the keyboard-only functions, so the game
//! is fully playable without a keyboard. Taps on lemmings are handled by the
//! HUD.

use bevy::prelude::*;

use crate::ViewCamera;
use crate::menu::AppState;

/// Finger travel (logical pixels) below which a touch counts as a tap.
pub const TAP_SLOP: f32 = 12.0;

/// Camera turn per pixel of one-finger horizontal drag (radians).
const TURN_PER_PX: f32 = 0.006;
/// Camera travel per pixel of drag (grid units).
const MOVE_PER_PX: f32 = 0.02;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            touch_camera
                .before(crate::camera_controls)
                .run_if(in_state(AppState::Playing)),
        );
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
