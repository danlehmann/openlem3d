//! Touch controls: dragging moves the camera. The panel's buttons (camera,
//! pause, turn, release rate) cover the keyboard-only functions, so the game
//! is fully playable without a keyboard. Taps on lemmings are handled by the
//! HUD.

use std::collections::HashMap;

use l3d_engine::prelude::*;

use crate::menu::AppState;
use crate::{ViewCamera, World};

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
/// How far two fingers move together (pixels) for the twist so far to fall
/// to a third (`1/e`): a hand turns a little as it drags, and that alone
/// should not start a twist.
const TWIST_FORGET_PX: f32 = 100.0;
/// How far in front of the camera, towards the fingers, a twist turns the
/// world about (grid units). A fixed distance rather than the spot under the
/// fingers, which jumps between near blocks and far water as they move.
const PIVOT_DISTANCE: f32 = 8.0;

/// While a finger is down, the window forgets the mouse pointer's last
/// position, so that buttons are pressed where the finger is: Bevy's UI
/// uses the pointer's position when it has one, and the touch's only
/// otherwise (taps on skills did nothing while the pointer had last been
/// over the window). The real pointer does not move.
pub(crate) fn touch_over_mouse(w: &mut World) {
    let touches = &w.e.input.touches;
    if touches.iter().next().is_none() && !touches.any_just_released() {
        return;
    }
    w.e.window.set_physical_cursor_position(None);
}

/// The screen each finger now down (or just lifted) landed on, and whether
/// it landed on a button.
#[derive(Default)]
pub struct TouchStarts(HashMap<u64, (AppState, bool)>);

impl TouchStarts {
    /// Whether the finger landed on a button. Its button is no longer pressed
    /// when it lifts, but lifting it is not a tap on what lies under it.
    pub fn on_button(&self, id: u64) -> bool {
        self.0.get(&id).is_some_and(|&(_, button)| button)
    }
}

pub(crate) fn record_touch_starts(w: &mut World) {
    let touches = &w.e.input.touches;
    let starts = &mut w.touch_starts;
    starts
        .0
        .retain(|&id, _| touches.get_pressed(id).is_some() || touches.just_released(id));
    let button = w.ui.any_pressed();
    for t in touches.iter_just_pressed() {
        starts.0.insert(t.id(), (w.state, button));
    }
    // Fingers of a three-finger time dial are no taps when they lift, though
    // they may not have moved (counted as on a button).
    if touches.iter().count() >= DIAL_FINGERS {
        for start in starts.0.values_mut() {
            start.1 = true;
        }
    }
}

/// Fingers that make the time dial.
const DIAL_FINGERS: usize = 3;
/// Fingers that, tapped together, open or close the configuration (ours).
const SETTINGS_FINGERS: usize = 4;

/// The fingers down since the last time none were: the most at once, and
/// whether any left the tap radius. Set `tapped` the frame the last of a
/// four-finger tap lifts.
#[derive(Default)]
pub struct FourFingerTap {
    most: usize,
    moved: bool,
    tapped: bool,
}

/// Whether four fingers were just tapped together.
pub fn four_finger_tap(w: &World) -> bool {
    w.four_finger_tap.tapped
}

pub(crate) fn watch_four_finger_tap(w: &mut World) {
    let touches = &w.e.input.touches;
    let tap = &mut w.four_finger_tap;
    tap.tapped = false;
    let down = touches.iter().count();
    tap.most = tap.most.max(down);
    tap.moved |= touches
        .iter()
        .chain(touches.iter_just_released())
        .any(|t| t.distance().length() >= TAP_SLOP);
    if down == 0 && touches.any_just_released() {
        tap.tapped = tap.most == SETTINGS_FINGERS && !tap.moved;
        tap.most = 0;
        tap.moved = false;
    }
}
/// Sideways travel for each unit of speed on the time dial, and the dead
/// zone where the fingers landed, as fractions of the screen height (ours).
pub(crate) const DIAL_UNIT: f32 = 0.12;
pub(crate) const DIAL_REST: f32 = 0.02;
/// The fastest the dial runs time, forwards or back (fast-forward's pace).
pub(crate) const DIAL_MAX: f32 = crate::FAST_FORWARD_TICKS as f32;

/// Three fingers: a time dial (ours). Where they landed the level stands
/// still; moved right it runs forward, slowly at first and up to
/// fast-forward's pace, and moved left it runs back the same way.
/// Lifting a finger leaves it to the usual pause and fast-forward.
#[derive(Default)]
pub struct TimeDial {
    /// The fingers' mean x where the third landed, while the dial is held;
    /// it stays set until every finger is up.
    origin: Option<f32>,
}

/// The signed speed for fingers `dx` pixels right of where they landed.
fn dial_speed(dx: f32, height: f32) -> f32 {
    let rest = DIAL_REST * height;
    let v = ((dx.abs() - rest).max(0.0) / (DIAL_UNIT * height)).min(DIAL_MAX);
    v.copysign(dx)
}

/// Whether a finger that landed on this screen lifted. A finger that chose
/// something on one screen (landing on a level in the practice grid, say)
/// is then not also a tap on the next, which would skip it.
pub fn lifted(w: &World) -> bool {
    w.e.input
        .touches
        .iter_just_released()
        .any(|t| w.touch_starts.0.get(&t.id()).map(|s| s.0) == Some(w.state))
}

/// Fingers now down (`down`) or just lifted that landed on this screen, and
/// whether they landed on a button, with where they are as [`Ui::focus`]
/// measures it.
fn fingers(w: &World, down: bool) -> impl Iterator<Item = (bool, Vec2)> + '_ {
    let touches = &w.e.input.touches;
    let all: Box<dyn Iterator<Item = _>> = if down {
        Box::new(touches.iter())
    } else {
        Box::new(touches.iter_just_released())
    };
    all.filter_map(move |t| {
        let &(state, button) = w.touch_starts.0.get(&t.id())?;
        let p = w.screen.on_screen(t.position()) * w.e.window.scale_factor();
        (state == w.state).then_some((button, p))
    })
}

/// Whether a finger is down on `id`, having landed on a button.
pub fn held_over(w: &World, id: crate::ui::NodeId) -> bool {
    fingers(w, true).any(|(button, p)| button && w.ui.over(id, p))
}

/// Whether a finger that landed on a button just lifted on `id`: a tap on
/// it, or a press slid onto it. Slid off, it picks nothing.
pub fn lifted_over(w: &World, id: crate::ui::NodeId) -> bool {
    fingers(w, false).any(|(button, p)| button && w.ui.over(id, p))
}

/// Whether a finger is down that landed off the buttons.
pub fn held_off_buttons(w: &World) -> bool {
    fingers(w, true).any(|(button, _)| !button)
}

/// Whether a finger that landed off the buttons just lifted.
pub fn lifted_off_buttons(w: &World) -> bool {
    fingers(w, false).any(|(button, _)| !button)
}

/// How much bigger a choice's prompt and label are drawn while a finger
/// holds it (ours).
const HELD_SCALE: f32 = 1.2;

/// Choices' pictures (by the choice's button) and where they were placed,
/// to grow while held and put back.
#[derive(Default)]
pub struct HeldChoices(HashMap<crate::ui::NodeId, Vec<(crate::ui::NodeId, Rect)>>);

impl HeldChoices {
    /// Draws the choice `id`'s pictures grown about their middle while
    /// `held`, as placed otherwise.
    pub fn show(&mut self, ui: &mut crate::ui::Ui, id: crate::ui::NodeId, held: bool) {
        self.0.retain(|&k, _| ui.contains(k));
        let parts = self.0.entry(id).or_insert_with(|| ui.pictures_within(id));
        let Some(bounds) = parts
            .iter()
            .map(|&(_, r)| r)
            .reduce(|a, b| Rect::from_corners(a.min.min(b.min), a.max.max(b.max)))
        else {
            return;
        };
        let c = bounds.center();
        let k = if held { HELD_SCALE } else { 1.0 };
        for &(part, r) in parts.iter() {
            if let Some(n) = ui.get_mut(part) {
                n.at = Some(Rect::from_center_size(
                    c + (r.center() - c) * k,
                    r.size() * k,
                ));
            }
        }
    }
}

/// One finger: horizontal drag turns, vertical drag moves forward/back.
/// Two fingers: as [`two_fingers`]. Riding along, any sideways drag turns the
/// head, as an action-button drag does.
pub(crate) fn touch_camera(w: &mut World) {
    let active: Vec<_> = w.e.input.touches.iter().collect();
    // Three fingers turn the time dial; after it, the camera waits until
    // every finger is up. A fourth leaves the dial for a four-finger tap.
    if active.len() == DIAL_FINGERS && !w.ui.any_pressed() {
        let x = active.iter().map(|t| t.position().x).sum::<f32>() / active.len() as f32;
        let origin = *w.time_dial.origin.get_or_insert(x);
        w.game.dial = Some(dial_speed(x - origin, w.screen.size.y));
        w.touch_twist = Twist::default();
        return;
    }
    w.game.dial = None;
    if w.time_dial.origin.is_some() {
        if active.is_empty() {
            w.time_dial.origin = None;
        }
        return;
    }
    let (screen, camera) = (&w.screen, &w.camera);
    let twist = &mut w.touch_twist;
    if active.len() < 2 {
        *twist = Twist::default();
    }
    if w.ui.any_pressed() {
        return;
    }
    let Some(first) = active.first() else { return };
    // Ignore fingers that have not left the tap radius: they may be taps.
    if active.len() == 1 && first.distance().length() < TAP_SLOP {
        return;
    }
    let delta = active.iter().map(|t| t.delta()).sum::<Vec2>() / active.len() as f32;
    if w.lemming_cam.following().is_some() {
        w.view.look(delta.x * TURN_PER_PX);
        return;
    }
    if let [a, b, ..] = active[..] {
        let (now, before) = (
            (a.position(), b.position()),
            (a.previous_position(), b.previous_position()),
        );
        let fingers = TwoFingers {
            delta,
            spread: now.0.distance(now.1) - before.0.distance(before.1),
            twist: twist.turn(
                (before.1 - before.0).angle_to(now.1 - now.0),
                delta.length(),
            ),
            turning: false,
            toward: crate::screen_ray(screen, camera, screen.on_screen((now.0 + now.1) / 2.0)),
        };
        two_fingers(&mut w.view, &fingers);
        return;
    }
    let cam = &mut w.view;
    let fwd = cam.forward();
    cam.yaw += delta.x * TURN_PER_PX;
    cam.pos += fwd * delta.y * MOVE_PER_PX;
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
/// together back; twisting them turns the world, as if turning it with them,
/// about a point [`PIVOT_DISTANCE`] ahead towards them.
pub(crate) fn two_fingers(cam: &mut ViewCamera, f: &TwoFingers) {
    let fwd = cam.forward();
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    if f.turning {
        cam.yaw += f.delta.x * TURN_PER_PX;
        cam.pos += fwd * f.delta.y * MOVE_PER_PX;
    } else {
        cam.pos += (-right * f.delta.x + Vec3::Y * f.delta.y) * MOVE_PER_PX;
    }
    cam.pos += f.toward.unwrap_or(fwd) * f.spread * MOVE_PER_PX;
    if f.twist != 0.0 {
        let way = f.toward.unwrap_or(fwd);
        let flat = Vec2::new(way.x, way.z).normalize_or(Vec2::new(fwd.x, fwd.z));
        let spot = Vec2::new(cam.pos.x, cam.pos.z) + flat * PIVOT_DISTANCE;
        cam.orbit(spot.to_array(), f.twist);
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
    /// How far a twist by `angle`, as the fingers move `travel` pixels
    /// together, turns the world.
    pub fn turn(&mut self, angle: f32, travel: f32) -> f32 {
        if !self.on {
            self.total *= (-travel / TWIST_FORGET_PX).exp();
        }
        self.total += angle;
        self.on |= self.total.abs() > TWIST_START;
        if self.on { angle } else { 0.0 }
    }
}
