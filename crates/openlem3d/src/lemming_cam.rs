//! The virtual-lemming camera: the view looks out of one lemming's eyes at
//! its eye level, turning as it turns, as the original's face icon does.
//! Arm it with the face button, V or I, then click or tap a lemming. V or I again,
//! Esc or a preset camera key (1–4) returns to the previous view, as does
//! the lemming leaving play.

use bevy::prelude::*;
use l3d_sim::SUB;

use crate::hud::{Highlight, SelectedSkill};
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
        view.head = 0.0;
        view.nod = 0.0;
        *self = LemmingCam::Off;
    }

    /// Rides along with lemming `i` (switching from another one if already
    /// riding), keeping the view to return to.
    pub fn follow(&mut self, i: usize, view: &ViewCamera) {
        let saved = self.saved().unwrap_or((view.pos, view.yaw));
        *self = LemmingCam::Following { lemming: i, saved };
    }

    /// The face button (or V or I): with a lemming highlighted that is not
    /// the one ridden with, rides along with it at once; otherwise arms the
    /// camera, or turns it off when it is on.
    pub fn face(
        &mut self,
        highlight: &mut Highlight,
        selected: &mut SelectedSkill,
        view: &mut ViewCamera,
    ) {
        match highlight.lemming {
            Some(i) if self.following() != Some(i) => self.ride(i, highlight, selected, view),
            _ => self.toggle(view),
        }
    }

    /// Rides along with lemming `i` (switching from another one if already
    /// riding). Riding highlights the lemming and deselects the skill
    /// (verified).
    pub fn ride(
        &mut self,
        i: usize,
        highlight: &mut Highlight,
        selected: &mut SelectedSkill,
        view: &ViewCamera,
    ) {
        self.follow(i, view);
        highlight.on = true;
        highlight.lemming = Some(i);
        selected.0 = None;
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
/// How fast the head turns while a turn key, turn arrow or turn region is
/// held, in radians per second (ours; slow in the original, owner's
/// observation).
pub const HEAD_SPEED: f32 = 0.8;
/// The least distance between the eye and terrain straight ahead.
const WALL_CLEARANCE: f32 = 0.15;
/// A mining lemming's nod: how far the horizon moves (a fraction of the
/// screen's height), and ticks per nod, one swing of its pickaxe (the six
/// frames of cells 241–270).
const NOD: f32 = 0.04;
const NOD_PERIOD: f32 = 6.0;
/// The least distance between the eye and a deflector's diagonal face.
/// A walker turns there only once its centre reaches the face, so an eye
/// above its centre would look through the block.
const DEFLECTOR_CLEARANCE: f32 = 0.2;
/// Moves between two ticks longer than this (in units) are jumps, which the
/// eye takes at once rather than sweeping through the level.
const JUMP: f32 = 1.0;

/// The eye's places at the last two simulation steps. Between steps the view
/// slides from the earlier to the later, one step behind the lemming, so it
/// moves smoothly at any frame rate.
#[derive(Default)]
struct Trail {
    from: Vec3,
    to: Vec3,
    /// Fixed time at the latest step seen.
    stepped: std::time::Duration,
}

impl Trail {
    /// A trail resting at `eye`.
    fn at(eye: Vec3, stepped: std::time::Duration) -> Self {
        Trail {
            from: eye,
            to: eye,
            stepped,
        }
    }
}

/// `eye`, moved straight out from the diagonal face of any deflector it is
/// closer to than [`DEFLECTOR_CLEARANCE`] (or behind).
/// `eye`, moved back from terrain straight ahead along `dir` (at eye level
/// and a little below) to [`WALL_CLEARANCE`] from it, so that a lemming at
/// work right against a block, as a miner or basher is, does not see through
/// it.
fn off_walls(world: &l3d_sim::World, eye: Vec3, dir: l3d_sim::Dir) -> Vec3 {
    let [x, _, z] = dir.delta();
    let ahead = Vec3::new(x as f32, 0.0, z as f32);
    let solid = |p: Vec3| world.solid(p.to_array().map(|v| (v * SUB as f32).floor() as i32));
    let steps = 16;
    (0..=steps)
        .map(|k| WALL_CLEARANCE * k as f32 / steps as f32)
        .find(|&s| {
            [0.0, -0.15]
                .iter()
                .any(|&h| solid(eye + ahead * s + Vec3::Y * h))
        })
        .map_or(eye, |s| eye - ahead * (WALL_CLEARANCE - s))
}

fn off_deflectors(world: &l3d_sim::World, mut eye: Vec3) -> Vec3 {
    const DIAGONALS: [[i32; 2]; 4] = [[1, 1], [1, -1], [-1, 1], [-1, -1]];
    for n in DIAGONALS {
        let normal = Vec3::new(n[0] as f32, 0.0, n[1] as f32).normalize();
        // A point just behind the face, as seen from the eye: if it lies in
        // a deflector facing the eye, the face passes through that cell's
        // centre.
        let behind = eye - normal * DEFLECTOR_CLEARANCE;
        let p = behind.to_array().map(|v| (v * SUB as f32).floor() as i32);
        if world.deflector_normal(p) != Some(n) {
            continue;
        }
        let centre = behind.floor() + Vec3::splat(0.5);
        let out = (eye - centre).with_y(0.0).dot(normal);
        if out < DEFLECTOR_CLEARANCE {
            eye += normal * (DEFLECTOR_CLEARANCE - out);
        }
    }
    eye
}

pub(crate) fn toggle_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut cam: ResMut<LemmingCam>,
    mut highlight: ResMut<Highlight>,
    mut selected: ResMut<SelectedSkill>,
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
        cam.face(&mut highlight, &mut selected, &mut view);
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
    fixed: Res<Time<Fixed>>,
    game: Res<Game>,
    opts: Res<crate::Options>,
    mut started: Local<bool>,
    mut sway: Local<f32>,
    mut entered: Local<Option<usize>>,
    mut trail: Local<Trail>,
    mut cam: ResMut<LemmingCam>,
    mut highlight: ResMut<Highlight>,
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
        // As riding along always does.
        highlight.on = true;
        highlight.lemming = Some(i);
    }
    let Some(mut i) = cam.following() else {
        view.roll = 0.0;
        view.head = 0.0;
        view.nod = 0.0;
        *entered = None;
        return;
    };
    let Some(sim) = game.sim.as_ref() else { return };
    // The lemming ridden with gone (home, or dead), the view goes on at once
    // with the next lemming in play; with none, it stays where it is until
    // another comes out (verified for drowning: the lemming right behind
    // was taken each time; whether the rule is release order or nearest is
    // open).
    let mut switched = false;
    if sim.lemmings.get(i).is_none_or(|l| l.gone) {
        let n = sim.lemmings.len();
        let Some(next) = (1..=n)
            .map(|k| (i + k) % n)
            .find(|&j| !sim.lemmings[j].gone)
        else {
            view.roll = 0.0;
            return;
        };
        i = next;
        switched = true;
        cam.follow(i, &view);
        highlight.on = true;
        highlight.lemming = Some(i);
    }
    let l = &sim.lemmings[i];
    let feet = Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32));
    // Passing to another lemming, the head faces forward again (owner's
    // observation).
    if switched || *entered != Some(i) {
        view.head = 0.0;
        view.nod = 0.0;
    }
    // A turner looks where its arm points, the way it sends walkers
    // (verified in Practice "Turner": riding with a "Turn right" turner, the
    // original's view faces that side, not along the path it came on), and
    // the head turned to the side looks that much further round.
    let yaw = view.head
        + match l.state {
            l3d_sim::State::Turning { to } => to.yaw(),
            // A blocker stands side-on, as it is drawn (owner's observation).
            l3d_sim::State::Blocking => l.dir.clockwise().yaw(),
            _ => l.dir.yaw(),
        };
    let target = off_walls(
        &sim.world,
        off_deflectors(&sim.world, feet + Vec3::Y * EYE_HEIGHT),
        l.dir,
    );
    // Taken on from a lemming that left: straight into its eyes.
    if switched {
        *entered = Some(i);
        view.glide = None;
        view.pos = target;
        view.yaw = yaw;
        view.roll = 0.0;
        *trail = Trail::at(target, fixed.elapsed());
        return;
    }
    // Riding along with a new lemming: the camera first glides into its
    // eyes, the level waiting meanwhile.
    if *entered != Some(i) {
        *entered = Some(i);
        view.roll = 0.0;
        view.glide_to(target, yaw);
    }
    if view.glide.is_some() {
        *trail = Trail::at(target, fixed.elapsed());
        return;
    }
    // A fixed step has run since the last frame: the simulation's position
    // (unchanged if it did not step, paused or waiting) becomes the new end.
    if fixed.elapsed() != trail.stepped {
        *trail = if target.distance(trail.to) > JUMP {
            Trail::at(target, fixed.elapsed())
        } else {
            Trail {
                from: trail.to,
                to: target,
                stepped: fixed.elapsed(),
            }
        };
    }
    let k = 1.0 - (-FOLLOW_RATE * time.delta_secs()).exp();
    // A slight roll from side to side with the lemming's steps while it
    // walks, levelling out otherwise; held still while the game is paused
    // (owner: the view must not move then).
    let walking = l.state == l3d_sim::State::Walking;
    if walking && !game.paused {
        *sway = (*sway + time.delta_secs() * std::f32::consts::TAU / SWAY_PERIOD)
            .rem_euclid(std::f32::consts::TAU);
    }
    let roll = if walking {
        SWAY_ROLL.to_radians() * sway.sin()
    } else {
        0.0
    };
    view.roll += (roll - view.roll) * (1.0 - (-ROLL_RATE * time.delta_secs()).exp());
    // Nodding with each swing of a miner's pickaxe while it cracks the
    // block ahead (owner's observation; the size and timing are ours).
    view.nod = if l.state == l3d_sim::State::Mining && l.state_ticks < l3d_sim::MINE_CRACK_TICKS {
        let t = l.state_ticks as f32 + fixed.overstep_fraction();
        // Looking down onto the block: the horizon moves up.
        -NOD * (1.0 - (t * std::f32::consts::TAU / NOD_PERIOD).cos()) / 2.0
    } else {
        0.0
    };
    // Turn the short way round.
    let turn = (yaw - view.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    view.yaw += turn * k;
    view.pos = trail.from.lerp(trail.to, fixed.overstep_fraction());
}
