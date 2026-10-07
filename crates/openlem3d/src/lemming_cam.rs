//! The virtual-lemming camera: the view rides along behind one lemming at
//! its eye level, turning as it turns, as the original's face icon does.
//! Arm it with the face button, V or I, then click or tap a lemming. V or I again,
//! Esc or a preset camera key (1–4) returns to the previous view, as does
//! the lemming leaving play.

use bevy::prelude::*;
use l3d_sim::SUB;

use crate::menu::AppState;
use crate::{Game, ViewCamera};

pub struct LemmingCamPlugin;

impl Plugin for LemmingCamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LemmingCam>()
            .add_systems(
                Update,
                (toggle_keys.before(crate::menu::back_to_menu), follow.before(crate::camera_controls)).chain().run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), |mut cam: ResMut<LemmingCam>| *cam = LemmingCam::Off);
    }
}

/// Camera position and yaw to return to.
pub type SavedView = (Vec3, f32);

#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub enum LemmingCam {
    #[default]
    Off,
    /// Waiting for a click on a lemming.
    Picking(SavedView),
    Following { lemming: usize, saved: SavedView },
}

impl LemmingCam {
    pub fn following(&self) -> Option<usize> {
        match self {
            LemmingCam::Following { lemming, .. } => Some(*lemming),
            _ => None,
        }
    }

    /// Arms the camera, or turns it off (restoring the view) when it is on.
    pub fn toggle(&mut self, view: &mut ViewCamera) {
        if *self == LemmingCam::Off {
            *self = LemmingCam::Picking((view.pos, view.yaw));
        } else {
            self.turn_off(view);
        }
    }

    /// Turns the camera off and puts the view back where it was.
    fn turn_off(&mut self, view: &mut ViewCamera) {
        if let Some((pos, yaw)) = self.saved() {
            view.pos = pos;
            view.yaw = yaw;
            view.roll = 0.0;
        }
        *self = LemmingCam::Off;
    }

    /// Starts following lemming i if the camera is armed; returns whether
    /// it was.
    pub fn pick(&mut self, i: usize) -> bool {
        match *self {
            LemmingCam::Picking(saved) => {
                *self = LemmingCam::Following { lemming: i, saved };
                true
            }
            _ => false,
        }
    }

    pub fn picking(&self) -> bool {
        matches!(self, LemmingCam::Picking(_))
    }

    pub fn saved(&self) -> Option<SavedView> {
        match self {
            LemmingCam::Off => None,
            LemmingCam::Picking(s) | LemmingCam::Following { saved: s, .. } => Some(*s),
        }
    }
}

/// Eye position relative to the lemming's feet: behind it and above its
/// head, so its back is in view (the original's lemming-cam view shows the
/// walker from behind; offsets estimated).
const EYE_BEHIND: f32 = 1.4;
const EYE_HEIGHT: f32 = 0.65;
/// How quickly the camera catches up with the lemming (per second).
const FOLLOW_RATE: f32 = 10.0;
/// The walking sway: peak roll in degrees, and seconds per swing left and
/// back, one walk cycle (6 ticks; ours, after the owner's observation that
/// the original rolls a little while the lemming walks).
const SWAY_ROLL: f32 = 1.5;
const SWAY_PERIOD: f32 = 6.0 / l3d_sim::TICKS_PER_SECOND as f32;
/// How quickly the roll follows the sway, per second.
const ROLL_RATE: f32 = 12.0;

pub(crate) fn toggle_keys(mut keys: ResMut<ButtonInput<KeyCode>>, mut cam: ResMut<LemmingCam>, mut views: Query<&mut ViewCamera>) {
    let Ok(mut view) = views.single_mut() else { return };
    let presets = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    if keys.just_pressed(KeyCode::KeyV) || keys.just_pressed(KeyCode::KeyI) {
        cam.toggle(&mut view);
    } else if keys.just_pressed(KeyCode::Escape) && *cam != LemmingCam::Off {
        // Esc leaves the lemming view, not the level.
        keys.clear_just_pressed(KeyCode::Escape);
        cam.turn_off(&mut view);
    } else if keys.any_just_pressed(presets) {
        // The preset key itself places the camera.
        *cam = LemmingCam::Off;
    }
}

fn follow(
    time: Res<Time>,
    game: Res<Game>,
    opts: Res<crate::Options>,
    mut started: Local<bool>,
    mut sway: Local<f32>,
    mut cam: ResMut<LemmingCam>,
    mut views: Query<&mut ViewCamera>,
) {
    let Ok(mut view) = views.single_mut() else { return };
    // `--follow N`: ride with lemming N as soon as it appears.
    if let Some(i) = opts.follow
        && !*started
        && game.sim.as_ref().is_some_and(|s| s.lemmings.len() > i)
    {
        *started = true;
        *cam = LemmingCam::Following { lemming: i, saved: (view.pos, view.yaw) };
    }
    let Some(i) = cam.following() else {
        view.roll = 0.0;
        return;
    };
    let lemming = game.sim.as_ref().and_then(|s| s.lemmings.get(i)).filter(|l| !l.gone);
    let Some(l) = lemming else {
        cam.turn_off(&mut view);
        return;
    };
    let feet = Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32));
    let yaw = l.dir.yaw();
    let forward = Vec3::new(-yaw.cos(), 0.0, -yaw.sin());
    // Behind the lemming, but no further back than the nearest wall.
    let eye = |behind: f32| feet - forward * behind + Vec3::Y * EYE_HEIGHT;
    let mut behind = EYE_BEHIND;
    if let Some((level, blocks, _)) = &game.terrain {
        while behind > 0.2 && crate::camera_blocked(level, blocks, eye(behind)) {
            behind -= 0.1;
        }
    }
    let target = eye(behind);
    let k = 1.0 - (-FOLLOW_RATE * time.delta_secs()).exp();
    // A slight roll from side to side with the lemming's steps while it
    // walks, levelling out otherwise.
    let walking = l.state == l3d_sim::State::Walking;
    if walking {
        *sway = (*sway + time.delta_secs() * std::f32::consts::TAU / SWAY_PERIOD).rem_euclid(std::f32::consts::TAU);
    }
    let roll = if walking { SWAY_ROLL.to_radians() * sway.sin() } else { 0.0 };
    view.roll += (roll - view.roll) * (1.0 - (-ROLL_RATE * time.delta_secs()).exp());
    // Turn the short way round.
    let turn = (yaw - view.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    view.yaw += turn * k;
    view.pos = view.pos.lerp(target, k);
}
