//! The virtual-lemming camera: the view looks out of one lemming's eyes at
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
                (
                    toggle_keys.before(crate::menu::back_to_menu),
                    follow.before(crate::camera_controls),
                )
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), |mut cam: ResMut<LemmingCam>| {
                *cam = LemmingCam::Off
            });
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
    Following {
        lemming: usize,
        saved: SavedView,
    },
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

    /// Turns the camera off, gliding the view back to where it was.
    fn turn_off(&mut self, view: &mut ViewCamera) {
        if let (Some((pos, yaw)), Some(_)) = (self.saved(), self.following()) {
            view.glide_to(pos, yaw);
        } else if let Some((pos, yaw)) = self.saved() {
            view.pos = pos;
            view.yaw = yaw;
        }
        view.roll = 0.0;
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

    /// Rides along with lemming `i` (switching from another one if already
    /// riding), keeping the view to return to.
    pub fn follow(&mut self, i: usize, view: &ViewCamera) {
        let saved = self.saved().unwrap_or((view.pos, view.yaw));
        *self = LemmingCam::Following { lemming: i, saved };
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

/// The lemming's eyes above its feet: the view is through them (the
/// followed lemming itself is not drawn). Its sprite stands half a unit
/// tall; the eyes are near the top of the head.
const EYE_HEIGHT: f32 = 0.4;
/// How quickly the view turns to the lemming's heading (per second).
const FOLLOW_RATE: f32 = 10.0;
/// The walking sway: peak roll in degrees, and seconds per swing left and
/// back, one walk cycle (6 ticks; ours, after the owner's observation that
/// the original rolls a little while the lemming walks).
const SWAY_ROLL: f32 = 1.5;
const SWAY_PERIOD: f32 = 6.0 / l3d_sim::TICKS_PER_SECOND as f32;
/// How quickly the roll follows the sway, per second.
const ROLL_RATE: f32 = 12.0;

pub(crate) fn toggle_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut cam: ResMut<LemmingCam>,
    mut views: Query<&mut ViewCamera>,
) {
    let Ok(mut view) = views.single_mut() else {
        return;
    };
    let presets = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
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

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn follow(
    time: Res<Time>,
    game: Res<Game>,
    opts: Res<crate::Options>,
    mut started: Local<bool>,
    mut sway: Local<f32>,
    mut entered: Local<Option<usize>>,
    mut cam: ResMut<LemmingCam>,
    mut views: Query<&mut ViewCamera>,
) {
    let Ok(mut view) = views.single_mut() else {
        return;
    };
    // `--follow N`: ride with lemming N as soon as it appears.
    if let Some(i) = opts.follow
        && !*started
        && game.sim.as_ref().is_some_and(|s| s.lemmings.len() > i)
    {
        *started = true;
        *cam = LemmingCam::Following {
            lemming: i,
            saved: (view.pos, view.yaw),
        };
    }
    let Some(i) = cam.following() else {
        view.roll = 0.0;
        *entered = None;
        return;
    };
    let lemming = game
        .sim
        .as_ref()
        .and_then(|s| s.lemmings.get(i))
        .filter(|l| !l.gone);
    let Some(l) = lemming else {
        cam.turn_off(&mut view);
        *entered = None;
        return;
    };
    let feet = Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32));
    // A turner looks where its arm points, the way it sends walkers
    // (verified in Practice "Turner": riding with a "Turn right" turner, the
    // original's view faces that side, not along the path it came on).
    let yaw = match l.state {
        l3d_sim::State::Turning { to } => to.yaw(),
        _ => l.dir.yaw(),
    };
    let target = feet + Vec3::Y * EYE_HEIGHT;
    // Riding along with a new lemming: the camera first glides into its
    // eyes, the level waiting meanwhile.
    if *entered != Some(i) {
        *entered = Some(i);
        view.roll = 0.0;
        view.glide_to(target, yaw);
    }
    if view.glide.is_some() {
        return;
    }
    let k = 1.0 - (-FOLLOW_RATE * time.delta_secs()).exp();
    // A slight roll from side to side with the lemming's steps while it
    // walks, levelling out otherwise.
    let walking = l.state == l3d_sim::State::Walking;
    if walking {
        *sway = (*sway + time.delta_secs() * std::f32::consts::TAU / SWAY_PERIOD)
            .rem_euclid(std::f32::consts::TAU);
    }
    let roll = if walking {
        SWAY_ROLL.to_radians() * sway.sin()
    } else {
        0.0
    };
    view.roll += (roll - view.roll) * (1.0 - (-ROLL_RATE * time.delta_secs()).exp());
    // Turn the short way round.
    let turn = (yaw - view.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    view.yaw += turn * k;
    view.pos = target;
}
