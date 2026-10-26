//! Turns simulation lemmings into camera-facing sprites from the `LEMM.MHC`
//! atlas.
//!
//! Lemmings are pre-rendered from several angles; the sprite is chosen by
//! the lemming's heading relative to the camera ([`ViewAngle`]). Cell ranges
//! follow `docs/spec/lemmings.md` ("Frame groups"), where each is marked
//! verified or guessed.

use std::f32::consts::{FRAC_PI_4, TAU};

use l3d_engine::math::Vec3;
use l3d_sim::objects::ObjectKind;
use l3d_sim::{Simulation, State};

use crate::scene_build::{
    ATLAS_COLUMNS, BOMBNUMB_ATLAS_FIRST, CRACK_ATLAS_FIRST, CRACK_FRAMES, DOOR_ATLAS_FIRST,
    DOOR_FRAMES, EXIT_ID, LEMMING_CELL, LayerBuilder, TILE_ATLAS_FIRST, TRAP_ATLAS_CELLS,
    TRAP_ATLAS_FIRST, TRAP_FRAME,
};
use crate::scene_render::SceneSprites;
use l3d_formats::blk::{BlockSet, FaceDir};

/// Texels per world unit for lemming cells: a cell spans half a grid unit
/// (estimated from relative sizes in the original's camera 1 view of Fun 1;
/// unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = LEMMING_CELL as f32 * 2.0;
/// How far a climber is drawn back from the wall face it climbs, in units:
/// half a lemming cell's width, so the sprite stays out of the wall.
const CLIMBER_OFF_WALL: f32 = LEMMING_CELL as f32 / LEMMING_TEXELS_PER_UNIT / 2.0;
/// The countdown digits (32 texels) stand a quarter unit tall.
const DIGIT_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame: one, at 14 ticks per second (the
/// original's sprites change once per tick; `docs/spec/behaviour.md`).
const TICKS_PER_FRAME: u32 = 1;

/// The floater's umbrella cells.
const UMBRELLA_FIRST: u32 = 336;
/// How far a rope slide's rope runs above the straight line its rider's
/// feet follow, from the floor at the sender to the floor at the receiver,
/// (¾–1 unit in Practice "Rope Slide").
const ROPE_ABOVE_PATH: f32 = 0.75;

/// The height in units of the rope over a rope-slide rider whose feet are
/// drawn at height `feet`,
/// [`ROPE_ABOVE_PATH`] above the line its feet follow (the rope is drawn by
/// a run of ramp blocks parallel to that line, ¾–1 unit above it in
/// Practice "Rope Slide"; searching the blocks for it found the deck
/// instead just off the start).
fn rope_above(feet: f32) -> f32 {
    feet + ROPE_ABOVE_PATH
}
const UMBRELLA_FRAMES: u32 = 8;

/// Which way a lemming faces as seen by the camera, in eighths of a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewAngle {
    /// Walking towards the camera.
    Front,
    FrontRight,
    /// Walking to the screen's right.
    Right,
    BackRight,
    /// Walking away from the camera.
    Back,
    BackLeft,
    Left,
    FrontLeft,
}

impl ViewAngle {
    /// The angle for a lemming heading `lemming_yaw` seen by a camera facing
    /// `camera_yaw` (both in the camera-yaw convention: 0 = −X, π/2 = −Z).
    /// Quantised to 45° sectors (the original's quantisation is unmeasured).
    pub fn from_yaws(lemming_yaw: f32, camera_yaw: f32) -> Self {
        // 0 when the lemming walks the way the camera looks (seen from behind).
        let rel = (lemming_yaw - camera_yaw).rem_euclid(TAU);
        let sector = ((rel + FRAC_PI_4 / 2.0) / FRAC_PI_4) as u32 % 8;
        // Yaw grows clockwise seen from above, so +¼ turn heads to screen right.
        [
            ViewAngle::Back,
            ViewAngle::BackRight,
            ViewAngle::Right,
            ViewAngle::FrontRight,
            ViewAngle::Front,
            ViewAngle::FrontLeft,
            ViewAngle::Left,
            ViewAngle::BackLeft,
        ][sector as usize]
    }

    /// The angle seen in a mirror: left and right swapped.
    fn mirrored(self) -> Self {
        use ViewAngle::*;
        match self {
            FrontRight => FrontLeft,
            FrontLeft => FrontRight,
            Right => Left,
            Left => Right,
            BackRight => BackLeft,
            BackLeft => BackRight,
            v => v,
        }
    }

    /// Position going round the lemming from the front: 0 = front,
    /// 1 = front-right, … 4 = back, … 7 = front-left.
    fn round_index(self) -> u32 {
        match self {
            ViewAngle::Front => 0,
            ViewAngle::FrontRight => 1,
            ViewAngle::Right => 2,
            ViewAngle::BackRight => 3,
            ViewAngle::Back => 4,
            ViewAngle::BackLeft => 5,
            ViewAngle::Left => 6,
            ViewAngle::FrontLeft => 7,
        }
    }
}

/// How an action's cells are arranged by viewing angle.
#[derive(Clone, Copy)]
enum Angles {
    /// Five blocks, front to back round the lemming's left: the stored side
    /// view faces screen-left (verified by eye on the walker, cells 12–17,
    /// and the ¾ views 6–11 and 18–23); the right-hand angles mirror them.
    Five,

    /// Eight blocks going round the lemming from the front by its left, as
    /// the turner's: its block 2 (cells 44–50) faces screen-left and block 6
    /// (72–78) screen-right, the pointing arm towards the camera (verified
    /// by eye); the floater's too (its block 1, 301–305, looks and swings
    /// its legs to screen-left; the other order showed a floater facing away
    /// from where it then walked, owner's observation).
    EightByLeft,
    /// One block, the same from every side (smoke, sparks).
    One,
}

/// The order in which an action's frames are shown, one per step.
#[derive(Clone, Copy)]
enum Playback {
    /// Frames 0, 1, … in a loop.
    Loop,
    /// Frames 0, 1, …, the last, then back down, in a loop: for poses that
    /// sweep one way through the frames (the turner's and blocker's head,
    /// the floater's legs), which a loop would snap back.
    PingPong,
    /// Frames 0, 1, …, then the last frame held.
    Once,
    /// The listed frame indices in a loop, the front and back views drawn
    /// mirrored (verified for falls); the side views as stored, which face
    /// the way the lemming goes (mirrored, it fell looking backwards).
    MirroredCycle(&'static [u32]),
    /// The listed frame indices in a loop, every view as stored.
    Cycle(&'static [u32]),
}

/// One action's animation: `frames` cells per angle block starting at `first`.
#[derive(Clone, Copy)]
struct Anim {
    first: u32,
    frames: u32,
    angles: Angles,
    playback: Playback,
}

const fn anim(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim {
        first,
        frames,
        angles,
        playback: Playback::Loop,
    }
}

const fn ping_pong(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim {
        first,
        frames,
        angles,
        playback: Playback::PingPong,
    }
}

const fn once(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim {
        first,
        frames,
        angles,
        playback: Playback::Once,
    }
}

/// Falling frame order: a ping-pong over the five frames, starting in the
/// middle.
const FALL_CYCLE: &[u32] = &[2, 3, 4, 3, 2, 1, 0, 1];

/// Sliding frame order: a ping-pong over the five frames, the arms rocking
/// one way and back (verified frame by frame on Practice "Slippery").
const SLIDE_CYCLE: &[u32] = &[0, 1, 2, 3, 4, 3, 2, 1];

/// The animation for a state (cell ranges from `docs/spec/lemmings.md`).
fn anim_for(state: State) -> Anim {
    use Angles::*;
    match state {
        State::Walking => anim(0, 6, Five),
        // Not found yet: lemmings walk into the exit.
        State::Exiting => anim(0, 6, Five),
        State::Turning { .. } => ping_pong(30, 7, EightByLeft),
        State::Blocking => ping_pong(86, 7, Five),
        State::Falling { .. } => Anim {
            first: 399,
            frames: 5,
            angles: Five,
            playback: Playback::MirroredCycle(FALL_CYCLE),
        },
        State::Digging => anim(161, 8, Five),
        State::Bashing => anim(201, 8, Five),
        State::Mining => anim(241, 6, Five),
        // The puff of smoke left by the blast; the swelling before it is
        // drawn from the fuse (see `build`).
        State::Exploding => once(534, 4, One),
        State::Floating => ping_pong(296, 5, EightByLeft),
        // Arms spread on ice: the pose with arms spread and one leg kicking
        // up, played as a ping-pong (both verified: a lemming sliding away
        // from the camera matched cells 558–562, the back view, in this
        // order, one frame per tick).
        State::Sliding => Anim {
            first: 538,
            frames: 5,
            angles: Five,
            playback: Playback::Cycle(SLIDE_CYCLE),
        },
        // Hanging from the handle by one raised hand: the floater's pose
        // (owner's picture of the original).
        State::OnRope { .. } => ping_pong(296, 5, EightByLeft),
        State::Flying { .. } | State::Bouncing { .. } => Anim {
            first: 399,
            frames: 5,
            angles: Five,
            playback: Playback::MirroredCycle(FALL_CYCLE),
        },
        State::Building { .. } => anim(344, 5, Five),
        // Out of bricks: a shrug with the empty sack (cells 474–488).
        State::Shrugging => once(474, 3, Five),
        State::Drowning => once(369, 6, Five),
        State::Splatting => once(429, 3, Five),
        // Lying still on the back, feet up (cells 429–443, the first frame:
        // owner's observation, no animation; see `build`).
        State::Stunned => once(429, 3, Five),
        State::Zapped | State::Electrocuted | State::Trapped => anim(514, 2, Five),
        // Reaching up the wall and pulling the knees up (cells 121–160).
        State::Climbing => anim(121, 8, Five),
        // Standing on the pad facing the camera (observed); see `build`.
        State::Teleporting { .. } => once(0, 1, One),
    }
}

/// The atlas cell and mirroring for an animation at a view angle and time.
fn cell_for(a: Anim, view: ViewAngle, ticks: u32) -> (u32, bool) {
    let step = ticks / TICKS_PER_FRAME;
    let (frame, flip) = match a.playback {
        Playback::Loop => (step % a.frames, false),
        Playback::PingPong => {
            let k = step % (2 * a.frames - 2).max(1);
            (k.min(2 * a.frames - 2 - k), false)
        }
        Playback::Once => (step.min(a.frames - 1), false),
        Playback::MirroredCycle(cycle) => (cycle[step as usize % cycle.len()], true),
        Playback::Cycle(cycle) => (cycle[step as usize % cycle.len()], false),
    };
    let (block, mirror) = match a.angles {
        Angles::EightByLeft => ((8 - view.round_index()) % 8, false),
        Angles::One => (0, false),
        Angles::Five => {
            let r = view.round_index();
            // Round positions 1–3 (the right) mirror the stored left-hand
            // views 7–5.
            match r {
                0 | 4 => (r, false),
                1..=3 => (r, true),
                _ => (8 - r, false),
            }
        }
    };
    (
        a.first + block * a.frames + frame,
        mirror != (flip && matches!(view, ViewAngle::Front | ViewAngle::Back)),
    )
}

/// The squasher's frames, a tick each from when it fires: it slams down
/// (frame 7 is fully down) and rises again. Recorded in the original
/// (`docs/spec/behaviour.md`, "Deaths").
const SQUASHER_FRAMES: [u32; 10] = [3, 7, 7, 6, 5, 4, 3, 2, 1, 0];
/// The bear trap's frames, a tick each from when it fires: the jaws snap
/// shut (frame 7) and open again (recorded).
const BEAR_TRAP_FRAMES: [u32; 9] = [7, 7, 7, 5, 4, 3, 2, 1, 0];

/// The flame-blower's jet, `t` ticks after it fired: full length for
/// [`JET_FULL_TICKS`], then shrinking back and gone at [`JET_TICKS`]
/// (recorded in the original, 2026-10-24, with the trap under a block as
/// real levels place it). Its frames are the sheet's eight jets, 64×32
/// texels in the top and bottom halves of frames 0–3.
fn flame_jet(b: &mut LayerBuilder, o: &l3d_sim::objects::Object, t: u32, eye: Vec3, tex: [f32; 2]) {
    if t >= JET_TICKS {
        return;
    }
    let length = if t < JET_FULL_TICKS {
        1.0
    } else {
        (JET_TICKS - t) as f32 / (JET_TICKS - JET_FULL_TICKS) as f32
    };
    let jet = t % 8;
    beam(
        b,
        o,
        eye,
        tex,
        (jet / 2, jet % 2),
        length,
        JET_HEIGHT,
        JET_THICKNESS,
    );
}

/// The laser's beam, `t` ticks after it fired: on while its lemming flashes
/// as an X-ray (the owner's picture of "Lemlab"), cycling through the
/// sheet's four beams, the tops of frames 0–3.
fn laser_beam(
    b: &mut LayerBuilder,
    o: &l3d_sim::objects::Object,
    t: u32,
    eye: Vec3,
    tex: [f32; 2],
) {
    if t < LASER_TICKS {
        beam(
            b,
            o,
            eye,
            tex,
            (t % 4, 0),
            1.0,
            LASER_HEIGHT,
            LASER_THICKNESS,
        );
    }
}

/// A trap's beam across its cell along Z, `height` above the floor, turned
/// about its own axis to face the camera at `eye`, drawn from the half
/// (0 top, 1 bottom) of trap frame `frame.0`; `length` (0–1) of it shows,
/// from its −Z end. Both beams run across the corridor, side-on in the
/// owner's pictures of "Raiders of the Lost Lemming" and "Lemlab", and
/// along the view in recordings looking along Z; that they always run
/// along Z is inferred (the trap blocks seen were turned 0 and 1).
#[allow(clippy::too_many_arguments)]
fn beam(
    b: &mut LayerBuilder,
    o: &l3d_sim::objects::Object,
    eye: Vec3,
    tex: [f32; 2],
    frame: (u32, u32),
    length: f32,
    height: f32,
    thickness: f32,
) {
    let cell = TRAP_ATLAS_FIRST + frame.0;
    let (cx, cy) = (
        (cell % ATLAS_COLUMNS) * LEMMING_CELL,
        (cell / ATLAS_COLUMNS) * LEMMING_CELL + frame.1 * TRAP_FRAME / 2,
    );
    let mid = Vec3::new(
        o.cell[0] as f32 + 0.5,
        crate::units(o.surface) + height,
        o.cell[2] as f32 + 0.5,
    );
    let from = mid - Vec3::Z * (BEAM_LENGTH / 2.0);
    let to = from + Vec3::Z * (BEAM_LENGTH * length);
    let side = (to - from).cross(eye - mid).normalize_or_zero() * (thickness / 2.0);
    let corners = [from + side, to + side, to - side, from - side].map(|v| v.to_array());
    let s = TRAP_FRAME as f32;
    b.quad(
        corners,
        [cx as f32, cy as f32, s * length, s / 2.0],
        tex,
        1.0,
    );
}

/// A beam's length, units; the flame-blower's jet's thickness and height
/// above the floor (about a lemming's knee) and the laser's (its waist),
/// read from the owner's pictures; how long the jet lasts at full length
/// and in all, and the laser beam (recorded).
const BEAM_LENGTH: f32 = 1.2;
const JET_THICKNESS: f32 = 0.6;
const JET_HEIGHT: f32 = 0.3;
const LASER_THICKNESS: f32 = 0.45;
const LASER_HEIGHT: f32 = 0.45;
const LASER_TICKS: u32 = 5;
const JET_FULL_TICKS: u32 = 11;
const JET_TICKS: u32 = 18;
/// How fast a lemming caught by a flame-blower rises into the block over
/// it, units a tick, and when it is gone (recorded: about 6 ticks to the
/// block half a unit up).
const JET_LIFT: f32 = 0.09;
const JET_LIFT_TICKS: u32 = 8;

/// The rope rider's scale: its 64-texel frame is a lemming hanging from its
/// pulley, half a unit tall.
const ROPE_RIDER_TEXELS_PER_UNIT: f32 = 128.0;

/// The ticks, from when a squasher or bear trap fires, over which the
/// caught lemming shows as a shrinking remnant once the trap opens, and the
/// remnant's scale at first (recorded: about half size, gone about 14
/// ticks in).
const REMNANT_TICKS: (u32, u32) = (3, 14);
const REMNANT_SCALE: f32 = 0.55;

/// Interactive objects other than trampolines (`TRAPS` frames, one unit
/// tall; the weird trap's, a whole lemming figure, half a unit, lemming
/// size as recorded). Bear traps and squashers rest on frame 0 and play
/// their recorded frames when they fire; the weird trap shows only while it
/// plays; flame-blowers and lasers show only while firing. Which frames are
/// idle is read off the sheets.
fn objects(b: &mut LayerBuilder, sim: &Simulation, eye: Vec3, tex: [f32; 2]) {
    let Some(kind) = sim.object_kind else { return };
    let texels_per_unit = if kind == ObjectKind::WeirdTrap {
        128.0
    } else {
        64.0
    };
    let last = TRAP_ATLAS_CELLS - 1;
    for o in &sim.objects {
        let firing = o.busy > 0;
        let elapsed = (l3d_sim::TRAP_BUSY_TICKS - o.busy) as usize;
        let played = |seq: &[u32]| {
            if firing {
                seq.get(elapsed).copied().unwrap_or(0)
            } else {
                0
            }
        };
        let frame = match kind {
            // Trampolines are pads in the static scene; one a lemming has just
            // touched plays its dip (frames 1–3 of its colour) on top, flat
            // just above the static pad. Rope-slide ends are not drawn at
            // all ([L3DEdit]).
            ObjectKind::Trampoline => {
                if o.busy > 0 {
                    let colour = if o.value >= 0x64 { 4 } else { 0 };
                    let cell =
                        TRAP_ATLAS_FIRST + colour + (l3d_sim::PAD_BUSY_TICKS - o.busy + 1).min(3);
                    let (cx, cy) = (
                        (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                        (cell / ATLAS_COLUMNS) * LEMMING_CELL,
                    );
                    let y = crate::units(o.surface) + 0.008;
                    let (x0, z0) = (o.cell[0] as f32, o.cell[2] as f32);
                    let (x1, z1) = (x0 + 1.0, z0 + 1.0);
                    let s = TRAP_FRAME as f32;
                    b.quad(
                        [[x0, y, z1], [x1, y, z1], [x1, y, z0], [x0, y, z0]],
                        [cx as f32, cy as f32, s, s],
                        tex,
                        1.0,
                    );
                }
                continue;
            }
            ObjectKind::RopeSlide => continue,
            ObjectKind::Squasher => played(&SQUASHER_FRAMES),
            ObjectKind::BearTrap => played(&BEAR_TRAP_FRAMES),
            // Unseen until it fires; the caught lemming then sinks a frame
            // per tick, down to the bare chains, and it is gone again
            // (recorded).
            ObjectKind::WeirdTrap if !firing || elapsed as u32 > last => continue,
            ObjectKind::WeirdTrap => elapsed as u32,
            // The lemming is electrocuted while the beam is on.
            ObjectKind::Laser => {
                if firing {
                    laser_beam(b, o, elapsed as u32, eye, tex);
                }
                continue;
            }
            ObjectKind::FlameBlower => {
                if firing {
                    flame_jet(b, o, elapsed as u32, eye, tex);
                }
                continue;
            }
            // Teleporter and spring pads are in the static scene too; a
            // spring shows its launch (frames 1–7, side on) while busy.
            ObjectKind::Teleporter => continue,
            // A sender (even value) stands as the spring at rest, frame 1;
            // the receiver is the flat target in the static scene (owner:
            // the sender is not drawn as the target).
            ObjectKind::Spring if o.busy == 0 && o.value % 2 == 0 => 1,
            ObjectKind::Spring if o.busy == 0 => continue,
            ObjectKind::Spring => 1 + (l3d_sim::SPRING_BUSY_TICKS - o.busy).min(6),
        };
        let cell = TRAP_ATLAS_FIRST + frame;
        let (cx, cy) = (
            (cell % ATLAS_COLUMNS) * LEMMING_CELL,
            (cell / ATLAS_COLUMNS) * LEMMING_CELL,
        );
        let anchor = [
            o.cell[0] as f32 + 0.5,
            crate::units(o.surface),
            o.cell[2] as f32 + 0.5,
        ];
        let size = TRAP_FRAME as f32;
        b.sprite_scaled(
            anchor,
            [cx as f32, cy as f32, size, size],
            tex,
            1,
            0.0,
            texels_per_unit,
        );
    }
}

/// Gap between the drawn door and the exit block's face.
const DOOR_OFFSET: f32 = 0.004;

/// An exit's door: its cell, the doorway's outward normal `[x, z]`, its
/// face's shading, and which of the [`DOOR_FRAMES`] pictures it shows (0:
/// closed, as the block itself is textured; 3: open).
#[derive(Clone, Copy, Debug)]
pub struct Door {
    pub cell: [i32; 3],
    pub normal: [i32; 2],
    pub brightness: f32,
    pub frame: u32,
}

/// The level's exit doors, closed.
pub fn doors(level: &l3d_formats::level::Level, blocks: &l3d_formats::blk::BlockSet) -> Vec<Door> {
    let shading = blocks
        .defs
        .get(EXIT_ID)
        .map_or(0, |d| d.face(l3d_formats::blk::FaceDir::PosZ).shading);
    level
        .cells()
        .filter(|(.., b, _)| b.id as usize == EXIT_ID && !b.is_empty())
        .map(|(x, y, z, b, _)| Door {
            cell: [x as i32, y as i32, z as i32],
            // The doorway is the +Z face turned with the block: 0 → +Z, 1 → +X,
            // 2 → −Z, 3 → −X (verified).
            normal: [[0, 1], [1, 0], [0, -1], [-1, 0]][(b.rotation % 4) as usize],
            brightness: 1.0 - (shading.min(8) as f32) * 0.07,
            frame: 0,
        })
        .collect()
}

/// One tick of the doors: a door with a lemming going in opens at once
/// (closed, half open, open on successive ticks) and stays open while
/// lemmings keep coming; then it swings shut a picture per tick (observed
/// in the original's Practice "Blocker" demo: open within about 0.1 s,
/// shut over about 0.15 s).
pub fn step_doors(doors: &mut [Door], sim: &Simulation) {
    for d in doors {
        let face = [
            d.cell[0] as f32 + 0.5 + d.normal[0] as f32 * 0.5,
            d.cell[2] as f32 + 0.5 + d.normal[1] as f32 * 0.5,
        ];
        let entering = sim.lemmings.iter().any(|l| {
            let p = l.pos.map(crate::units);
            !l.gone
                && l.state == State::Exiting
                && (p[0] - face[0]).abs() < 0.8
                && (p[2] - face[1]).abs() < 0.8
                && (p[1] - d.cell[1] as f32).abs() < 1.0
        });
        d.frame = match (entering, d.frame) {
            (true, 0) => 2,
            (true, _) => DOOR_FRAMES - 1,
            (false, f) => f.saturating_sub(1),
        };
    }
}

/// How wide a teleporting lemming is drawn, `state_ticks` into it.
fn teleport_width(t: u32) -> f32 {
    use l3d_sim::{
        TELEPORT_APPEAR as APPEAR, TELEPORT_AWAY as AWAY, TELEPORT_STAND as STAND,
        TELEPORT_VANISH as VANISH,
    };
    if t < STAND {
        1.0
    } else if t < STAND + VANISH {
        1.0 - (t - STAND + 1) as f32 / (VANISH + 1) as f32
    } else if t < STAND + VANISH + AWAY {
        0.0
    } else if t < STAND + VANISH + AWAY + APPEAR {
        (t - STAND - VANISH - AWAY + 1) as f32 / (APPEAR + 1) as f32
    } else {
        1.0
    }
}

/// The teleporter sparkle: frames 4–7 of its `TRAPS` sheet (the pad is
/// 0–3), drawn so the column stands about half a unit tall, half again a
/// lemming's height (observed).
const SPARKLE_FIRST: u32 = 4;
const SPARKLE_FRAMES: u32 = 4;
const SPARKLE_TEXELS_PER_UNIT: f32 = 112.0;

/// Where a teleporting lemming's sparkle shows, `state_ticks` into it: over
/// the pad it left for the first half of its time away, then over the one
/// it arrives at (observed: about 0.5 s each).
fn sparkle_at(t: u32, from: [i32; 3], to: [i32; 3]) -> Option<[i32; 3]> {
    use l3d_sim::{TELEPORT_AWAY as AWAY, TELEPORT_STAND as STAND, TELEPORT_VANISH as VANISH};
    let away = t.checked_sub(STAND + VANISH).filter(|&a| a < AWAY)?;
    Some(if away < AWAY / 2 { from } else { to })
}

/// Ticks over which a lemming going into an exit shrinks away (observed:
/// about 0.45 s), and how far it moves on meanwhile, in units.
const EXIT_SHRINK_TICKS: u32 = 6;
const EXIT_SHRINK_DEPTH: f32 = 0.2;

/// The storm cloud's cells: growing (524–526), striking with bolts
/// (527–529), shrinking away (530–533).
const ZAP_CLOUD_FIRST: u32 = 524;
/// Height of a standing lemming's head top above its feet, in units (the
/// figure fills about 110 of the cell's 128 pixels).
const LEMMING_HEAD: f32 = 110.0 / LEMMING_TEXELS_PER_UNIT;
/// The storm cloud's scale: about 2.4 lemming cells across, as recorded.
const ZAP_CLOUD_TEXELS_PER_UNIT: f32 = LEMMING_TEXELS_PER_UNIT / 2.4;
/// The smoke puff's cells, big to small.
const PUFF_FIRST: u32 = 534;

/// The ticks into an electrocution or a boundary zap at which the lemming
/// shows as an X-ray skeleton (recorded in the original, 2026-10-24: the
/// liquid flashes it three times; at the boundary each burst of bolts
/// strikes it twice and the last once).
fn xray(state: State, t: u32) -> bool {
    match state {
        State::Electrocuted => t < 5 && t % 2 == 0,
        State::Zapped => matches!(t, 1 | 3 | 7 | 9 | 13),
        _ => false,
    }
}

/// The smoke puff a zapped or electrocuted lemming leaves, as a frame of
/// [`PUFF_FIRST`], two ticks each (recorded): it replaces the lemming.
fn puff(state: State, t: u32) -> Option<u32> {
    let from = match state {
        State::Electrocuted => l3d_sim::ELECTROCUTE_TICKS - 8,
        State::Zapped => l3d_sim::ZAP_TICKS - 8,
        _ => return None,
    };
    (t >= from).then(|| ((t - from) / 2).min(3))
}

/// The storm cloud over a lemming zapped at the boundary, `t` ticks in, as
/// a cell (recorded): it gathers, strikes in three bursts of bolts three
/// ticks apart, and shrinks away while the lemming is smoke.
fn zap_cloud(t: u32) -> Option<u32> {
    let frame = match t {
        0 => 1,
        1..=3 | 7..=9 | 13..=15 => 3 + (t - 1) % 6,
        4..=6 | 10..=12 => 2,
        16..=19 => 6 + t - 16,
        _ => return None,
    };
    Some(ZAP_CLOUD_FIRST + frame)
}

/// How far crack quads stand off the block's faces, in units.
const CRACK_OFFSET: f32 = 0.004;
/// Pieces flying off a bashed block: bursts starting this many ticks into a
/// stroke, pieces per burst, and ticks each flies (measured in the Practice
/// "Basher" demo: bursts at about 1, 9, 18 and 25 ticks, 25–30 pieces,
/// gone after 6–7 ticks).
const CHUNK_BURSTS: [u32; 4] = [1, 9, 17, 25];
const CHUNKS_PER_BURST: u32 = 25;
const CHUNK_TICKS: u32 = 7;
/// A piece's flight per tick, in units (rough, from the same demo: out of
/// the hole away from the block, a kick upwards, then falling faster).
const CHUNK_OUT: (f32, f32) = (0.08, 0.2);
const CHUNK_UP: (f32, f32) = (0.06, 0.18);
const CHUNK_GRAVITY: f32 = 0.03;

/// A pseudo-random fraction in [0, 1) from integers (the effects are drawn
/// from the simulation's state alone, so they replay identically).
fn noise(keys: [u32; 5]) -> f32 {
    let mut h = 0x9E37_79B9u32;
    for k in keys {
        h = (h ^ k)
            .wrapping_mul(0x85EB_CA6B)
            .rotate_left(13)
            .wrapping_mul(0xC2B2_AE35);
    }
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The unit outward normal of the diagonal face of a deflector in `cell`
/// at height `y` (units), if the cell holds one there. The face runs through
/// the cell's centre.
fn deflector_face(sim: &Simulation, cell: [i32; 3], y: f32) -> Option<[f32; 2]> {
    let sub = l3d_sim::SUB as f32;
    // A point in each quarter of the cell; the solid half holds one of them.
    [[0.25, 0.25], [0.25, 0.75], [0.75, 0.25], [0.75, 0.75]]
        .into_iter()
        .find_map(|[x, z]| {
            let at = |c: i32, v: f32| ((c as f32 + v) * sub) as i32;
            sim.world
                .deflector_normal([at(cell[0], x), (y * sub) as i32, at(cell[2], z)])
        })
        .map(|n| n.map(|v| v as f32 * std::f32::consts::FRAC_1_SQRT_2))
}

/// The black cracks of `OVERLAY.000` (frame `frame`) over the sides of the
/// segments in `mask` of `cell`.
fn draw_cracks(
    b: &mut LayerBuilder,
    sim: &Simulation,
    cell: [i32; 3],
    mask: u8,
    frame: u32,
    tex: [f32; 2],
) {
    let crack = CRACK_ATLAS_FIRST + frame.min(CRACK_FRAMES - 1);
    let c = cell.map(|v| v as f32);
    let (cx, cy) = (
        (crack % ATLAS_COLUMNS) * LEMMING_CELL,
        (crack / ATLAS_COLUMNS) * LEMMING_CELL,
    );
    // Each segment's slice of the overlay: segment s (0 at the bottom) is
    // the face's texel rows (3 − s)·16 to (4 − s)·16, on each of the
    // block's sides there: the cube faces it fills, and a deflector's
    // diagonal face.
    for s in (0..4).filter(|s| mask & (1 << s) != 0) {
        let rect = [cx as f32, cy as f32 + (3 - s) as f32 * 16.0, 64.0, 16.0];
        let y = c[1] + s as f32 * 0.25;
        for n in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
            let centre = [
                c[0] + 0.5 + n[0] * (0.5 + CRACK_OFFSET),
                y,
                c[2] + 0.5 + n[1] * (0.5 + CRACK_OFFSET),
            ];
            side_cracks(b, sim, centre, n, 1.0, rect, tex);
        }
        if let Some(n) = deflector_face(sim, cell, y + 0.125) {
            let centre = [
                c[0] + 0.5 + n[0] * CRACK_OFFSET,
                y,
                c[2] + 0.5 + n[1] * CRACK_OFFSET,
            ];
            side_cracks(b, sim, centre, n, std::f32::consts::SQRT_2, rect, tex);
        }
    }
}

/// Pieces across and up a segment's side for its cracks.
const SIDE_CRACK_PIECES: (usize, usize) = (16, 4);

/// The cracks (`rect` of the atlas) over a segment's side facing `normal`, a
/// quarter unit tall and `width` wide from its bottom middle `centre`: in
/// pieces, each drawn only where the block is solid just behind it, so that
/// on a ramp or wedge they keep to its sloped outline instead of hanging in
/// the air beside it.
fn side_cracks(
    b: &mut LayerBuilder,
    sim: &Simulation,
    centre: [f32; 3],
    normal: [f32; 2],
    width: f32,
    rect: [f32; 4],
    tex: [f32; 2],
) {
    let (cols, rows) = SIDE_CRACK_PIECES;
    let (pw, ph) = (width / cols as f32, 0.25 / rows as f32);
    let (rw, rh) = (rect[2] / cols as f32, rect[3] / rows as f32);
    // Right as seen from the front, as `vertical_quad` lays it out.
    let right = [normal[1], -normal[0]];
    let at = |v: f32| (v * l3d_sim::SUB as f32).floor() as i32;
    for i in 0..cols {
        let along = -width / 2.0 + (i as f32 + 0.5) * pw;
        let x = centre[0] + right[0] * along;
        let z = centre[2] + right[1] * along;
        // Just behind the face.
        let inside = 0.02 + CRACK_OFFSET;
        let (ix, iz) = (x - normal[0] * inside, z - normal[1] * inside);
        for j in 0..rows {
            let y = centre[1] + j as f32 * ph;
            if sim
                .world
                .block_at([at(ix), at(y + ph / 2.0), at(iz)])
                .is_none()
            {
                continue;
            }
            b.vertical_quad(
                [x, y, z],
                normal,
                pw,
                ph,
                [
                    rect[0] + i as f32 * rw,
                    rect[1] + rect[3] - (j + 1) as f32 * rh,
                    rw,
                    rh,
                ],
                tex,
                1.0,
            );
        }
    }
}

/// The mine effects: the cracks spreading over the block ahead of a miner
/// while it cracks it, and pieces of it flying back out of its face (the
/// original shows both, owner's observation; timing, counts and speeds
/// ours, after the basher's).
fn mine_effects(b: &mut LayerBuilder, sim: &Simulation, blocks: Option<&BlockSet>, tex: [f32; 2]) {
    // The cracks a miner left when it was switched part way (they stay).
    for &(cell, mask, t) in &sim.world.mine_wear {
        draw_cracks(
            b,
            sim,
            cell,
            mask,
            t * CRACK_FRAMES / (l3d_sim::MINE_CRACK_TICKS - 1),
            tex,
        );
    }
    for (i, l) in sim.lemmings.iter().enumerate().filter(|(_, l)| !l.gone) {
        let Some((cells, t)) = sim.mine_target(l) else {
            continue;
        };
        for &(cell, mask) in &cells {
            let frame = t * CRACK_FRAMES / (l3d_sim::MINE_CRACK_TICKS - 1);
            draw_cracks(b, sim, cell, mask, frame, tex);
        }
        let Some(&(cell, _)) = cells.first() else {
            continue;
        };
        let id = sim.world.block(cell).map_or(0, |(b, _)| b.id as usize);
        let tile = blocks
            .and_then(|bs| bs.defs.get(id))
            .map_or(0xFF, |d| d.face(FaceDir::PosX).texture);
        if tile == 0xFF {
            continue;
        }
        let tile = tile as u32;
        let tile_cell = TILE_ATLAS_FIRST + tile / 4;
        let (tx, ty) = (
            (tile_cell % ATLAS_COLUMNS) * LEMMING_CELL + (tile % 2) * 64,
            (tile_cell / ATLAS_COLUMNS) * LEMMING_CELL + (tile / 2 % 2) * 64,
        );
        let d = l.dir.delta().map(|v| v as f32);
        let (out, side) = ([-d[0], -d[2]], [d[2], -d[0]]);
        let c = cell.map(|v| v as f32);
        let face = [c[0] + 0.5 - d[0] * 0.5, c[2] + 0.5 - d[2] * 0.5];
        let feet = crate::units(l.pos[1]);
        let stroke = sim.tick as u32 / 72;
        for (burst, &start) in MINE_BURSTS.iter().enumerate() {
            if !(start..start + CHUNK_TICKS).contains(&t) {
                continue;
            }
            let age = (t - start) as f32;
            for k in 0..CHUNKS_PER_BURST / 2 {
                let r = |salt: u32| noise([i as u32, 0x313E + stroke, burst as u32, k, salt]);
                let v_out = CHUNK_OUT.0 + r(0) * (CHUNK_OUT.1 - CHUNK_OUT.0);
                let v_up = CHUNK_UP.0 + r(1) * (CHUNK_UP.1 - CHUNK_UP.0);
                let v_side = (r(2) - 0.5) * 0.16;
                let lateral = (r(3) - 0.5) * 0.8;
                let y0 = feet - 0.25 + r(4) * 0.75;
                let along =
                    |a: usize| face[a] + out[a] * v_out * age + side[a] * (lateral + v_side * age);
                let y = y0 + v_up * age - CHUNK_GRAVITY * age * age / 2.0;
                let size = 6.0 + (r(5) * 5.0).floor();
                let (sx, sy) = (
                    (r(6) * (64.0 - size)).floor(),
                    (r(7) * (64.0 - size)).floor(),
                );
                b.sprite_scaled(
                    [along(0), y, along(1)],
                    [tx as f32 + sx, ty as f32 + sy, size, size],
                    tex,
                    1,
                    0.0,
                    96.0,
                );
            }
        }
    }
}

/// Bursts of pieces from a block a miner cracks, ticks into the cracking
/// (about one a swing: 0.6 s apart in the original).
const MINE_BURSTS: [u32; 4] = [1, 10, 19, 28];

/// The dig effects: the cracks spreading over the segment a digger is
/// digging out, on its sides and over its top, all of them over the time it
/// takes (`docs/spec/behaviour.md`, Digger: removed after a crack
/// animation).
fn dig_effects(b: &mut LayerBuilder, sim: &Simulation, blocks: Option<&BlockSet>, tex: [f32; 2]) {
    // The cracks a digger left when it stopped part way (owner's
    // observation: they stay).
    for &(cell, mask, t) in &sim.world.dig_wear {
        let frame = t * CRACK_FRAMES / l3d_sim::DIG_TICKS;
        draw_cracks(b, sim, cell, mask, frame, tex);
        let crack = CRACK_ATLAS_FIRST + frame.min(CRACK_FRAMES - 1);
        let at = [
            ((crack % ATLAS_COLUMNS) * LEMMING_CELL) as f32,
            ((crack / ATLAS_COLUMNS) * LEMMING_CELL) as f32,
        ];
        draw_top_cracks(b, sim, cell, mask, at, tex);
    }
    for (i, l) in sim.lemmings.iter().enumerate().filter(|(_, l)| !l.gone) {
        let Some((cell, mask, t)) = sim.dig_target(l) else {
            continue;
        };
        dig_debris(b, sim, blocks, i, l, cell, mask, t, tex);
        let frame = t * CRACK_FRAMES / l3d_sim::DIG_TICKS;
        draw_cracks(b, sim, cell, mask, frame, tex);
        let crack = CRACK_ATLAS_FIRST + frame.min(CRACK_FRAMES - 1);
        let (cx, cy) = (
            (crack % ATLAS_COLUMNS) * LEMMING_CELL,
            (crack / ATLAS_COLUMNS) * LEMMING_CELL,
        );
        draw_top_cracks(b, sim, cell, mask, [cx as f32, cy as f32], tex);
        draw_bottom_cracks(b, sim, cell, mask, [cx as f32, cy as f32], tex);
    }
}

/// The cracks (at `at` in the atlas) under what a digger is digging out of
/// `cell` (its segments in `mask`) where that has air below it: on the
/// sloped underside of an upside-down wedge it cuts into, or under a block
/// hanging in the air, facing down, in pieces that each lie wholly on such
/// an underside.
fn draw_bottom_cracks(
    b: &mut LayerBuilder,
    sim: &Simulation,
    cell: [i32; 3],
    mask: u8,
    at: [f32; 2],
    tex: [f32; 2],
) {
    let sub = l3d_sim::SUB;
    let (low, high) = (
        cell[1] * sub + mask.trailing_zeros() as i32 * sub / 4,
        cell[1] * sub + (8 - mask.leading_zeros()) as i32 * sub / 4,
    );
    // The underside at a point of the cell, in sub-units: the lowest solid
    // height in the band, if there is air below it.
    let underside = |fx: f32, fz: f32| {
        let inside = |c: i32, f: f32| c * sub + ((f * sub as f32) as i32).clamp(0, sub - 1);
        let (x, z) = (inside(cell[0], fx), inside(cell[2], fz));
        let solid = |y: i32| sim.world.block_at([x, y, z]).is_some();
        // Found in steps, then down to where the brick starts.
        let mut bottom = (low..high).step_by(4).find(|&y| solid(y))?;
        while bottom > low && solid(bottom - 1) {
            bottom -= 1;
        }
        (!solid(bottom - 1)).then_some(bottom)
    };
    let n = TOP_CRACK_PIECES as f32;
    let piece = 64.0 / n;
    for i in 0..TOP_CRACK_PIECES {
        for j in 0..TOP_CRACK_PIECES {
            let (fx0, fx1) = (i as f32 / n, (i + 1) as f32 / n);
            let (fz0, fz1) = (j as f32 / n, (j + 1) as f32 / n);
            let e = 0.01 / n;
            let corners = [
                (fx0 + e, fz0 + e),
                (fx1 - e, fz0 + e),
                (fx1 - e, fz1 - e),
                (fx0 + e, fz1 - e),
            ];
            let Some(heights) = corners
                .iter()
                .map(|&(fx, fz)| underside(fx, fz).map(|y| crate::units(y) - CRACK_OFFSET))
                .collect::<Option<Vec<f32>>>()
            else {
                continue;
            };
            let (x0, x1) = (cell[0] as f32 + fx0, cell[0] as f32 + fx1);
            let (z0, z1) = (cell[2] as f32 + fz0, cell[2] as f32 + fz1);
            // Wound to face down.
            b.quad(
                [
                    [x0, heights[0], z0],
                    [x1, heights[1], z0],
                    [x1, heights[2], z1],
                    [x0, heights[3], z1],
                ],
                [
                    at[0] + i as f32 * piece,
                    at[1] + j as f32 * piece,
                    piece,
                    piece,
                ],
                tex,
                1.0,
            );
        }
    }
}

/// Pieces across a cell (each way) of the cracks over a digger's segment.
const TOP_CRACK_PIECES: usize = 8;

/// The cracks (at `at` in the atlas) over the top of what a digger is
/// digging out of `cell` (its segments in `mask`): lying on the block's
/// surface within that band, so that they follow a ramp's slope where the
/// ramp is cut, and only where there is something to dig under every corner
/// of a piece.
fn draw_top_cracks(
    b: &mut LayerBuilder,
    sim: &Simulation,
    cell: [i32; 3],
    mask: u8,
    at: [f32; 2],
    tex: [f32; 2],
) {
    let sub = l3d_sim::SUB;
    let (low, high) = (
        cell[1] * sub + mask.trailing_zeros() as i32 * sub / 4,
        cell[1] * sub + (8 - mask.leading_zeros()) as i32 * sub / 4,
    );
    // The surface at a point of the cell, in sub-units: the band's top where
    // the block reaches it, the band's bottom where it doesn't reach that.
    let surface = |fx: f32, fz: f32| {
        let inside = |c: i32, f: f32| c * sub + ((f * sub as f32) as i32).clamp(0, sub - 1);
        sim.world
            .surface_below(inside(cell[0], fx), high, inside(cell[2], fz))
            .0
    };
    let height = |fx: f32, fz: f32| crate::units(surface(fx, fz).clamp(low, high)) + CRACK_OFFSET;
    let n = TOP_CRACK_PIECES as f32;
    let piece = 64.0 / n;
    for i in 0..TOP_CRACK_PIECES {
        for j in 0..TOP_CRACK_PIECES {
            let (fx0, fx1) = (i as f32 / n, (i + 1) as f32 / n);
            let (fz0, fz1) = (j as f32 / n, (j + 1) as f32 / n);
            // Every corner on something in the band (a hair inside the
            // piece): a corner over air would be pulled down to the band's
            // bottom, hanging the piece into the air at the edge of the
            // hole or under a wedge.
            let e = 0.01 / n;
            let on_material = [
                (fx0 + e, fz0 + e),
                (fx1 - e, fz0 + e),
                (fx0 + e, fz1 - e),
                (fx1 - e, fz1 - e),
            ]
            .iter()
            .all(|&(fx, fz)| surface(fx, fz) > low);
            if !on_material {
                continue;
            }
            let (x0, x1) = (cell[0] as f32 + fx0, cell[0] as f32 + fx1);
            let (z0, z1) = (cell[2] as f32 + fz0, cell[2] as f32 + fz1);
            b.quad(
                [
                    [x0, height(fx0, fz1), z1],
                    [x1, height(fx1, fz1), z1],
                    [x1, height(fx1, fz0), z0],
                    [x0, height(fx0, fz0), z0],
                ],
                [
                    at[0] + i as f32 * piece,
                    at[1] + j as f32 * piece,
                    piece,
                    piece,
                ],
                tex,
                1.0,
            );
        }
    }
}

/// Bursts of pieces of the block's top texture thrown up and out on all
/// sides from around a digger's feet, ticks into each segment it digs
/// (owner: the digger throws debris; the bursts, counts and speeds are ours,
/// after the basher's).
const DIG_BURSTS: [u32; 4] = [2, 12, 22, 32];
const DIG_PIECES_PER_BURST: u32 = 12;

#[allow(clippy::too_many_arguments)]
fn dig_debris(
    b: &mut LayerBuilder,
    sim: &Simulation,
    blocks: Option<&BlockSet>,
    i: usize,
    l: &l3d_sim::Lemming,
    cell: [i32; 3],
    mask: u8,
    t: u32,
    tex: [f32; 2],
) {
    let id = sim.world.block(cell).map_or(0, |(b, _)| b.id as usize);
    let tile = blocks
        .and_then(|bs| bs.defs.get(id))
        .map_or(0xFF, |d| d.face(FaceDir::PosY).texture);
    if tile == 0xFF {
        return;
    }
    let tile = tile as u32;
    let tile_cell = TILE_ATLAS_FIRST + tile / 4;
    let (tx, ty) = (
        (tile_cell % ATLAS_COLUMNS) * LEMMING_CELL + (tile % 2) * 64,
        (tile_cell / ATLAS_COLUMNS) * LEMMING_CELL + (tile / 2 % 2) * 64,
    );
    let top = cell[1] as f32 + (mask.trailing_zeros() + 1) as f32 * 0.25;
    let feet = l.pos.map(crate::units);
    let stroke = l.state_ticks / l3d_sim::DIG_TICKS;
    for (burst, &start) in DIG_BURSTS.iter().enumerate() {
        if !(start..start + CHUNK_TICKS).contains(&t) {
            continue;
        }
        let age = (t - start) as f32;
        for k in 0..DIG_PIECES_PER_BURST {
            let r = |salt: u32| noise([i as u32, 0xD16 + stroke, burst as u32, k, salt]);
            let angle = r(0) * std::f32::consts::TAU;
            let v_out = 0.5 * (CHUNK_OUT.0 + r(1) * (CHUNK_OUT.1 - CHUNK_OUT.0));
            let v_up = CHUNK_UP.0 + r(2) * (CHUNK_UP.1 - CHUNK_UP.0);
            let from = 0.1 + r(3) * 0.15;
            let (dx, dz) = (angle.cos(), angle.sin());
            let x = feet[0] + dx * (from + v_out * age);
            let z = feet[2] + dz * (from + v_out * age);
            let y = top + v_up * age - CHUNK_GRAVITY * age * age / 2.0;
            let size = 6.0 + (r(4) * 5.0).floor();
            let (sx, sy) = (
                (r(5) * (64.0 - size)).floor(),
                (r(6) * (64.0 - size)).floor(),
            );
            b.sprite_scaled(
                [x, y, z],
                [tx as f32 + sx, ty as f32 + sy, size, size],
                tex,
                1,
                0.0,
                96.0,
            );
        }
    }
}

/// The bash effects (`docs/spec/behaviour.md`, Basher): the black cracks of
/// `OVERLAY.000` spreading over the sides of the segments being bashed, one
/// frame every 2 ticks, and bursts of pieces cut from the block's texture
/// flying out of the hole.
fn bash_effects(b: &mut LayerBuilder, sim: &Simulation, blocks: Option<&BlockSet>, tex: [f32; 2]) {
    for (i, l) in sim.lemmings.iter().enumerate().filter(|(_, l)| !l.gone) {
        let Some((cell, mask, t)) = sim.bash_target(l) else {
            continue;
        };
        let c = cell.map(|v| v as f32);
        draw_cracks(b, sim, cell, mask, t / 2, tex);
        // The pieces: bits of the block's side texture.
        let id = sim.world.block(cell).map_or(0, |(b, _)| b.id as usize);
        let tile = blocks
            .and_then(|bs| bs.defs.get(id))
            .map_or(0xFF, |d| d.face(FaceDir::PosX).texture);
        if tile == 0xFF {
            continue;
        }
        let tile = tile as u32;
        let tile_cell = TILE_ATLAS_FIRST + tile / 4;
        let (tx, ty) = (
            (tile_cell % ATLAS_COLUMNS) * LEMMING_CELL + (tile % 2) * 64,
            (tile_cell / ATLAS_COLUMNS) * LEMMING_CELL + (tile / 2 % 2) * 64,
        );
        let d = l.dir.delta().map(|v| v as f32);
        let (out, side) = ([-d[0], -d[2]], [d[2], -d[0]]);
        let bottom = c[1] + mask.trailing_zeros() as f32 * 0.25;
        // The side facing the basher: the cube face, or a deflector's
        // diagonal (crossing the cell's centre line).
        let inset = if deflector_face(sim, cell, bottom + 0.125)
            .is_some_and(|n| n[0] * d[0] + n[1] * d[2] < 0.0)
        {
            0.0
        } else {
            0.5
        };
        let face = [c[0] + 0.5 - d[0] * inset, c[2] + 0.5 - d[2] * inset];
        let stroke = (l.state_ticks - l3d_sim::BASH_CRACK_START) / l3d_sim::BASH_TICKS;
        for (burst, &start) in CHUNK_BURSTS.iter().enumerate() {
            if !(start..start + CHUNK_TICKS).contains(&t) {
                continue;
            }
            let age = (t - start) as f32;
            for k in 0..CHUNKS_PER_BURST {
                let r = |salt: u32| noise([i as u32, stroke, burst as u32, k, salt]);
                let v_out = CHUNK_OUT.0 + r(0) * (CHUNK_OUT.1 - CHUNK_OUT.0);
                let v_up = CHUNK_UP.0 + r(1) * (CHUNK_UP.1 - CHUNK_UP.0);
                let v_side = (r(2) - 0.5) * 0.16;
                let lateral = (r(3) - 0.5) * 0.8;
                let y0 = bottom + r(4) * 0.5;
                let along =
                    |a: usize| face[a] + out[a] * v_out * age + side[a] * (lateral + v_side * age);
                let y = y0 + v_up * age - CHUNK_GRAVITY * age * age / 2.0;
                let size = 6.0 + (r(5) * 5.0).floor();
                let (sx, sy) = (
                    (r(6) * (64.0 - size)).floor(),
                    (r(7) * (64.0 - size)).floor(),
                );
                b.sprite_scaled(
                    [along(0), y, along(1)],
                    [tx as f32 + sx, ty as f32 + sy, size, size],
                    tex,
                    1,
                    0.0,
                    96.0,
                );
            }
        }
    }
}

/// A bomber or a splatted lemming bursting, `t` ticks in, at `at`, drawn
/// from `rect` (the fully swollen bomber): it swells a little more, then
/// flies apart into `pieces` small copies of itself that shoot up, spread
/// and rain down past the ground (seen in the original side-on, 2026-10-23:
/// about a dozen, half size, rising about 2 units in 0.8 s, gone after
/// about 2½ s; speeds ours, read from those frames). A splat bursts the
/// same way into about eight (recorded, 2026-10-24).
fn burst(
    b: &mut LayerBuilder,
    i: usize,
    t: u32,
    at: [f32; 3],
    rect: [f32; 4],
    tex: [f32; 2],
    (pieces, out, scale): (u32, f32, f32),
) {
    if t < BURST_SWELL_TICKS {
        b.sprite_scaled(at, rect, tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT / BURST_SWELL);
        return;
    }
    let age = (t - BURST_SWELL_TICKS) as f32;
    for k in 0..pieces {
        let r = |salt: u32| noise([i as u32, 0xB0B, k, salt, 0]);
        let angle = r(0) * std::f32::consts::TAU;
        let v_out = out * (0.3 + 0.7 * r(1));
        let v_up = BURST_UP.0 + r(2) * (BURST_UP.1 - BURST_UP.0);
        let p = [
            at[0] + angle.cos() * v_out * age,
            at[1] + 0.2 + v_up * age - BURST_GRAVITY * age * age / 2.0,
            at[2] + angle.sin() * v_out * age,
        ];
        b.sprite_scaled(p, rect, tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT / scale);
    }
}

/// A bursting bomber: ticks it swells further first, by how much, then its
/// pieces, their speed outwards and upwards (units a tick) and their fall.
const BURST_SWELL_TICKS: u32 = 2;
const BURST_SWELL: f32 = 1.3;
const BURST_PIECES: u32 = 12;
const SPLAT_PIECES: u32 = 8;
/// A splat's pieces fan out wider: about 1½ units in ten ticks (recorded).
const SPLAT_OUT: f32 = 0.14;
/// A splat's pieces are bigger than a bomber's: about two thirds of the
/// swollen lemming (recorded).
const SPLAT_SCALE: f32 = 0.7;
const BURST_OUT: f32 = 0.04;
const BURST_UP: (f32, f32) = (0.2, 0.36);
const BURST_GRAVITY: f32 = 0.033;

/// One of the level's other cameras, drawn in the world where it is.
pub struct CameraMarker {
    /// The camera's number, 1–4.
    pub number: u32,
    pub pos: Vec3,
    /// Facing, as `ViewCamera::yaw`.
    pub yaw: f32,
}

/// The film camera's cells: lens towards the viewer, side (lens to the
/// screen's left as stored), back. The three-quarter cells between them
/// (564, 566) never show (verified).
const CAMERA_FRONT: u32 = 563;
const CAMERA_SIDE: u32 = 565;
const CAMERA_BACK: u32 = 567;
/// How far above a camera its number is centred (measured 0.39–0.43).
const CAMERA_NUMBER_RISE: f32 = 0.4;

/// A facing's quarter turn, 0–3, the quarters centred 11.25° to the left of
/// the four rotations (fitted to the original's camera markers).
fn camera_quarter(yaw: f32) -> i32 {
    ((yaw + std::f32::consts::PI / 16.0) / std::f32::consts::FRAC_PI_2).floor() as i32 & 3
}

/// The camera cell, and whether it is mirrored, for a camera facing `yaw`
/// seen from a view facing `viewer_yaw`. Only the two facings' quarters
/// count, not the bearing between them (verified).
fn camera_cell(yaw: f32, viewer_yaw: f32) -> (u32, bool) {
    match (camera_quarter(yaw) - camera_quarter(viewer_yaw)) & 3 {
        2 => (CAMERA_FRONT, false),
        3 => (CAMERA_SIDE, false),
        1 => (CAMERA_SIDE, true),
        _ => (CAMERA_BACK, false),
    }
}

/// The other cameras: the film camera, centred on the camera's position,
/// with its number (`BOMBNUMB`) above it. Neither animates.
fn camera_markers(b: &mut LayerBuilder, markers: &[CameraMarker], viewer_yaw: f32, tex: [f32; 2]) {
    let rect = |cell: u32, size: f32, mirror: bool| {
        let (cx, cy) = (
            ((cell % ATLAS_COLUMNS) * LEMMING_CELL) as f32,
            ((cell / ATLAS_COLUMNS) * LEMMING_CELL) as f32,
        );
        if mirror {
            [cx + size, cy, -size, size]
        } else {
            [cx, cy, size, size]
        }
    };
    for m in markers {
        let (cell, mirror) = camera_cell(m.yaw, viewer_yaw);
        let size = LEMMING_CELL as f32;
        let body = m.pos - Vec3::Y * (size / LEMMING_TEXELS_PER_UNIT / 2.0);
        b.sprite_scaled(
            body.to_array(),
            rect(cell, size, mirror),
            tex,
            1,
            0.0,
            LEMMING_TEXELS_PER_UNIT,
        );
        let number = m.pos + Vec3::Y * (CAMERA_NUMBER_RISE - 32.0 / DIGIT_TEXELS_PER_UNIT / 2.0);
        b.sprite_scaled(
            number.to_array(),
            rect(BOMBNUMB_ATLAS_FIRST + m.number - 1, 32.0, false),
            tex,
            1,
            0.0,
            DIGIT_TEXELS_PER_UNIT,
        );
    }
}

/// The way a lemming's sprite faces. A turner stands facing the walkers
/// coming up behind it, the way it came (seen in the original's "Turner"
/// demo; drawn the other way, turners showed their backs to the cameras of
/// "Take a Dive"). A blocker stands side-on, its outstretched arms pointing
/// back the way it came and on the way it was going (owner's observation).
pub fn facing(l: &l3d_sim::Lemming) -> l3d_sim::Dir {
    match l.state {
        State::Turning { .. } => l.dir.reverse(),
        State::Blocking => l.dir.clockwise(),
        _ => l.dir,
    }
}

/// Builds this frame's lemming sprites for a camera facing `camera_yaw`,
/// leaving out lemming `eyes`, the one the view looks out of, and marking
/// lemming `highlight` with an arrow; and the other cameras' markers.
/// `drawn` is where each lemming is drawn, in units, and the way it faces
/// (see `Game::drawn_pos` and `Game::turn_yaws`).
#[allow(clippy::too_many_arguments)]
pub fn build(
    sim: &Simulation,
    drawn: &[([f32; 3], f32)],
    blocks: Option<&BlockSet>,
    atlas_rows: u32,
    camera_yaw: f32,
    eye: Vec3,
    doors: &[Door],
    eyes: Option<usize>,
    highlight: Option<usize>,
    cameras: &[CameraMarker],
) -> SceneSprites {
    let lemmings = &sim.lemmings;
    let mut b = LayerBuilder::default();
    let tex = [
        (ATLAS_COLUMNS * LEMMING_CELL) as f32,
        (atlas_rows * LEMMING_CELL) as f32,
    ];
    objects(&mut b, sim, eye, tex);
    camera_markers(&mut b, cameras, camera_yaw, tex);
    bash_effects(&mut b, sim, blocks, tex);
    mine_effects(&mut b, sim, blocks, tex);
    dig_effects(&mut b, sim, blocks, tex);
    for d in doors.iter().filter(|d| d.frame > 0) {
        let cell = DOOR_ATLAS_FIRST + d.frame;
        let (cx, cy) = (
            (cell % ATLAS_COLUMNS) * LEMMING_CELL,
            (cell / ATLAS_COLUMNS) * LEMMING_CELL,
        );
        let n = d.normal.map(|v| v as f32);
        let c = [
            d.cell[0] as f32 + 0.5 + n[0] * (0.5 + DOOR_OFFSET),
            d.cell[1] as f32,
            d.cell[2] as f32 + 0.5 + n[1] * (0.5 + DOOR_OFFSET),
        ];
        b.vertical_quad(
            c,
            n,
            1.0,
            1.0,
            [cx as f32, cy as f32, 64.0, 64.0],
            tex,
            d.brightness,
        );
    }
    // Trapped lemmings are shown by the trap's own animation, but for the
    // remnant a squasher or bear trap leaves (see `REMNANT_TICKS`).
    // (or a flame-blower's lifted lemming).
    let remnant = matches!(
        sim.object_kind,
        Some(ObjectKind::Squasher | ObjectKind::BearTrap | ObjectKind::FlameBlower)
    );
    for (i, l) in lemmings
        .iter()
        .enumerate()
        .filter(|(i, l)| !l.gone && (l.state != State::Trapped || remnant) && Some(*i) != eyes)
    {
        let facing = facing(l);
        let yaw = drawn.get(i).map_or(facing.yaw(), |d| d.1);
        // A lemming caught by a flame-blower spins as it rises (owner, on
        // "Raiders of the Lost Lemming"; an eighth of a turn a tick is ours).
        let yaw = if l.state == State::Trapped && sim.object_kind == Some(ObjectKind::FlameBlower) {
            yaw + l.state_ticks as f32 * FRAC_PI_4
        } else {
            yaw
        };
        let view = ViewAngle::from_yaws(yaw, camera_yaw);
        let (cell, mirror) = match l.state {
            // The stored turner points to its right (cells 30–36, seen from
            // the front, hold out the lemming's right arm); one pointing
            // left is its mirror image. The reverse showed turners pointing
            // away from where they sent walkers.
            State::Turning { to } if to == facing.anticlockwise() => {
                let (cell, mirror) = cell_for(anim_for(l.state), view.mirrored(), l.state_ticks);
                (cell, !mirror)
            }
            State::Stunned => cell_for(anim_for(l.state), view, 0),
            State::Trapped => cell_for(anim_for(State::Walking), view, 0),
            // Sinking a frame every two ticks, gone after the last
            // (recorded).
            State::Drowning => cell_for(anim_for(l.state), view, l.state_ticks / 2),
            // Standing, flashing between itself and its X-ray (cells
            // 514–523 in pairs).
            State::Zapped | State::Electrocuted => cell_for(
                anim_for(l.state),
                view,
                if xray(l.state, l.state_ticks) {
                    TICKS_PER_FRAME
                } else {
                    0
                },
            ),
            // A miner walks down the block it has broken (owner's
            // observation).
            State::Mining if l.state_ticks >= l3d_sim::MINE_CRACK_TICKS => {
                cell_for(anim_for(State::Walking), view, l.state_ticks)
            }
            // Bursting: pieces of the fully swollen bomber (see `burst`).
            State::Exploding | State::Splatting => {
                cell_for(once(271, 5, Angles::Five), view, 4 * TICKS_PER_FRAME)
            }
            // A basher walks up to the next wall, before the first and
            // between blocks (owner's observation).
            State::Bashing if l.bash_walk_ticks > 0 => {
                cell_for(anim_for(State::Walking), view, l.bash_walk_ticks)
            }
            // A builder on its way to the middle of its cell carries the
            // sack already (owner's observation).
            State::Walking if matches!(l.pending, Some(State::Building { .. })) => {
                cell_for(anim(444, 6, Angles::Five), view, l.state_ticks)
            }
            // Each brick: the walk up onto it and along it with the sack
            // (444–473), as the simulation moves the builder, then the
            // laying motion over the rest of the 25-tick cycle.
            State::Building { .. } => {
                // In step with the bricks, the first of which comes
                // FIRST_BRICK_TICKS in, after a laying motion.
                let c = (l.state_ticks + l3d_sim::BUILD_TICKS - l3d_sim::FIRST_BRICK_TICKS)
                    % l3d_sim::BUILD_TICKS;
                let walk = l3d_sim::BRICK_WALK_TICKS as u32;
                if l.state_ticks > l3d_sim::FIRST_BRICK_TICKS && (1..=walk).contains(&c) {
                    cell_for(anim(444, 6, Angles::Five), view, c)
                } else {
                    let into = (c + l3d_sim::BUILD_TICKS - walk - 1) % l3d_sim::BUILD_TICKS;
                    let laying = into * 5 / (l3d_sim::BUILD_TICKS - walk);
                    cell_for(once(344, 5, Angles::Five), view, laying.min(4))
                }
            }
            // A bomber swells over the last ticks of its fuse (observed:
            // about 10 ticks after the last countdown digit).
            _ if l.fuse.is_some_and(|f| f <= l3d_sim::SWELL_TICKS) && !l.state.is_terminal() => {
                // Hand to the nose, holding it shut for most of the time
                // (frames 1–2), then swelling (3–4); the split is ours.
                let frame = match l3d_sim::SWELL_TICKS - l.fuse.unwrap_or(0) {
                    0 => 0,
                    1..=2 => 1,
                    3..=6 => 2,
                    7..=8 => 3,
                    _ => 4,
                };
                cell_for(once(271, 5, Angles::Five), view, frame * TICKS_PER_FRAME)
            }
            _ => cell_for(anim_for(l.state), view, l.state_ticks),
        };
        let (cx, cy) = (
            (cell % ATLAS_COLUMNS) * LEMMING_CELL,
            (cell / ATLAS_COLUMNS) * LEMMING_CELL,
        );
        let size = LEMMING_CELL as f32;
        let rect = if mirror {
            [cx as f32 + size, cy as f32, -size, size]
        } else {
            [cx as f32, cy as f32, size, size]
        };
        let mut anchor = drawn.get(i).map_or(l.pos.map(crate::units), |d| d.0);
        // A climber's centre is on the wall face; drawn there, half of it
        // would be inside the wall.
        if l.state == State::Climbing {
            let d = l.dir.delta();
            anchor[0] -= d[0] as f32 * CLIMBER_OFF_WALL;
            anchor[2] -= d[2] as f32 * CLIMBER_OFF_WALL;
        }
        // A rope rider is drawn from the rope slide's own sheet: the lemming
        // hanging from its pulley, five views from the front round its left
        // to the back, like a walker's (the sheet, and the original's
        // Practice "Rope Slide", 2026-10-25), the pulley at the rope.
        if matches!(l.state, State::OnRope { .. }) {
            let (cell, mirror) = cell_for(anim(TRAP_ATLAS_FIRST, 1, Angles::Five), view, 0);
            let (cx, cy) = (
                (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                (cell / ATLAS_COLUMNS) * LEMMING_CELL,
            );
            let s = TRAP_FRAME as f32;
            let rect = if mirror {
                [cx as f32 + s, cy as f32, -s, s]
            } else {
                [cx as f32, cy as f32, s, s]
            };
            anchor[1] = rope_above(anchor[1]) - s / ROPE_RIDER_TEXELS_PER_UNIT;
            b.sprite_scaled(anchor, rect, tex, 1, 0.0, ROPE_RIDER_TEXELS_PER_UNIT);
            continue;
        }
        // On a rope slide the raised hand, at the top of the cell, holds the
        // rope (owner's picture).
        if matches!(l.state, State::OnRope { .. }) {
            anchor[1] = rope_above(anchor[1]) - size / LEMMING_TEXELS_PER_UNIT;
        }
        if matches!(l.state, State::Exploding | State::Splatting) {
            let pieces = if l.state == State::Exploding {
                (BURST_PIECES, BURST_OUT, 0.5)
            } else {
                (SPLAT_PIECES, SPLAT_OUT, SPLAT_SCALE)
            };
            burst(&mut b, i, l.state_ticks, anchor, rect, tex, pieces);
            continue;
        }
        if l.state == State::Zapped
            && let Some(cell) = zap_cloud(l.state_ticks)
        {
            // The storm cloud over the lemming, its bolts reaching down to
            // the lemming's head (cells 524–533, view-independent).
            let (cx, cy) = (
                (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                (cell / ATLAS_COLUMNS) * LEMMING_CELL,
            );
            let over = [anchor[0], anchor[1] + LEMMING_HEAD, anchor[2]];
            b.sprite_scaled(
                over,
                [cx as f32, cy as f32, size, size],
                tex,
                1,
                0.0,
                ZAP_CLOUD_TEXELS_PER_UNIT,
            );
        }
        if let Some(frame) = puff(l.state, l.state_ticks) {
            let cell = PUFF_FIRST + frame;
            let (cx, cy) = (
                (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                (cell / ATLAS_COLUMNS) * LEMMING_CELL,
            );
            b.sprite_scaled(
                anchor,
                [cx as f32, cy as f32, size, size],
                tex,
                1,
                0.0,
                LEMMING_TEXELS_PER_UNIT,
            );
            continue;
        }
        // Teleporting: squeezed to a sliver, away, then stretched out again
        // (observed).
        let width = match l.state {
            State::Teleporting { to } => {
                if let Some(at) = sparkle_at(l.state_ticks, l.pos, to) {
                    let cell = TRAP_ATLAS_FIRST + SPARKLE_FIRST + l.state_ticks % SPARKLE_FRAMES;
                    let (cx, cy) = (
                        (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                        (cell / ATLAS_COLUMNS) * LEMMING_CELL,
                    );
                    let s = TRAP_FRAME as f32;
                    b.sprite_scaled(
                        at.map(crate::units),
                        [cx as f32, cy as f32, s, s],
                        tex,
                        1,
                        0.0,
                        SPARKLE_TEXELS_PER_UNIT,
                    );
                }
                teleport_width(l.state_ticks)
            }
            _ => 1.0,
        };
        // Exiting: shrinking into the doorway (observed), walking on into it.
        let (scale, anchor) = match l.state {
            State::Exiting => {
                let s = 1.0
                    - (l.state_ticks + 1).min(EXIT_SHRINK_TICKS) as f32 / EXIT_SHRINK_TICKS as f32;
                let [dx, _, dz] = l.dir.delta();
                let ahead = (1.0 - s) * EXIT_SHRINK_DEPTH;
                (
                    s,
                    [
                        anchor[0] + dx as f32 * ahead,
                        anchor[1],
                        anchor[2] + dz as f32 * ahead,
                    ],
                )
            }
            // Lifted on the flame-blower's jet into the block over it.
            State::Trapped if sim.object_kind == Some(ObjectKind::FlameBlower) => {
                let t = l.state_ticks;
                let s = if t < JET_LIFT_TICKS { 1.0 } else { 0.0 };
                (s, [anchor[0], anchor[1] + JET_LIFT * t as f32, anchor[2]])
            }
            State::Trapped => {
                let (from, to) = REMNANT_TICKS;
                let t = l.state_ticks;
                let s = if (from..to).contains(&t) {
                    REMNANT_SCALE * (to - t) as f32 / (to - from) as f32
                } else {
                    0.0
                };
                (s, anchor)
            }
            _ => (1.0, anchor),
        };
        if width <= 0.0 || scale <= 0.0 {
            continue;
        }
        b.sprite_squeezed(
            anchor,
            rect,
            tex,
            1,
            0.0,
            LEMMING_TEXELS_PER_UNIT / scale,
            width,
        );
        // The bomber's countdown, 5…1, over its head (`BOMBNUMB` digits).
        if let Some(fuse) = l.fuse.filter(|_| !l.state.is_terminal()) {
            let digit =
                5u32.saturating_sub((l3d_sim::FUSE_TICKS - fuse) / l3d_sim::FUSE_DIGIT_TICKS);
            if digit > 0 {
                let cell = BOMBNUMB_ATLAS_FIRST + digit - 1;
                let (cx, cy) = (
                    (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                    (cell / ATLAS_COLUMNS) * LEMMING_CELL,
                );
                let over = [
                    anchor[0],
                    anchor[1] + size / LEMMING_TEXELS_PER_UNIT,
                    anchor[2],
                ];
                b.sprite_scaled(
                    over,
                    [cx as f32, cy as f32, 32.0, 32.0],
                    tex,
                    1,
                    0.0,
                    DIGIT_TEXELS_PER_UNIT,
                );
            }
        }
        if Some(i) == highlight {
            // The highlight arrow over the lemming (`BOMBNUMB` cell 6).
            let cell = BOMBNUMB_ATLAS_FIRST + 6;
            let (cx, cy) = (
                (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                (cell / ATLAS_COLUMNS) * LEMMING_CELL,
            );
            let over = [
                anchor[0],
                anchor[1] + size / LEMMING_TEXELS_PER_UNIT,
                anchor[2],
            ];
            b.sprite_scaled(
                over,
                [cx as f32, cy as f32, 32.0, 32.0],
                tex,
                1,
                0.0,
                DIGIT_TEXELS_PER_UNIT,
            );
        }
        if l.state == State::Floating {
            // The umbrella is stored apart (cells 336–343), its handle at the
            // bottom of the cell, held by the floater's raised hand at the top
            // of the floater's cell (frame order unverified).
            let cell = UMBRELLA_FIRST + l.state_ticks % UMBRELLA_FRAMES;
            let (cx, cy) = (
                (cell % ATLAS_COLUMNS) * LEMMING_CELL,
                (cell / ATLAS_COLUMNS) * LEMMING_CELL,
            );
            let top = [
                anchor[0],
                anchor[1] + size / LEMMING_TEXELS_PER_UNIT,
                anchor[2],
            ];
            b.sprite_scaled(
                top,
                [cx as f32, cy as f32, size, size],
                tex,
                1,
                0.0,
                LEMMING_TEXELS_PER_UNIT,
            );
        }
    }
    SceneSprites {
        vertices: b.vertices,
        indices: b.indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn view_angles() {
        // Camera facing +Z (yaw 3π/2).
        let cam = 3.0 * FRAC_PI_2;
        assert_eq!(ViewAngle::from_yaws(3.0 * FRAC_PI_2, cam), ViewAngle::Back); // walking +Z
        assert_eq!(ViewAngle::from_yaws(FRAC_PI_2, cam), ViewAngle::Front); // walking −Z
        // Facing +Z, screen right is −X (yaw 0).
        assert_eq!(ViewAngle::from_yaws(0.0, cam), ViewAngle::Right);
        assert_eq!(ViewAngle::from_yaws(PI, cam), ViewAngle::Left);
    }

    #[test]
    fn camera_cells() {
        let deg = |d: f32| d.to_radians();
        // Facing each other: the lens.
        assert_eq!(camera_cell(PI, 0.0), (CAMERA_FRONT, false));
        // Facing the same way: the back.
        assert_eq!(camera_cell(deg(90.0), deg(90.0)), (CAMERA_BACK, false));
        // Turned from facing the viewer: +68° still the lens, +80.5° the
        // side as stored, −12.3° the side mirrored, −107.6° the back
        // (measured).
        assert_eq!(camera_cell(PI + deg(68.0), 0.0), (CAMERA_FRONT, false));
        assert_eq!(camera_cell(PI + deg(80.5), 0.0), (CAMERA_SIDE, false));
        assert_eq!(camera_cell(PI - deg(12.3), 0.0), (CAMERA_SIDE, true));
        assert_eq!(camera_cell(PI - deg(107.6), 0.0), (CAMERA_BACK, false));
    }

    #[test]
    fn walker_cells() {
        let walk = anim_for(State::Walking);
        assert_eq!(cell_for(walk, ViewAngle::Front, 0), (0, false));
        assert_eq!(cell_for(walk, ViewAngle::Right, 0), (12, true));
        assert_eq!(cell_for(walk, ViewAngle::Left, 0), (12, false));
        assert_eq!(
            cell_for(walk, ViewAngle::Back, 2 * TICKS_PER_FRAME),
            (26, false)
        );
        assert_eq!(cell_for(walk, ViewAngle::FrontLeft, 0), (6, false));
    }

    #[test]
    fn faller_cells() {
        let fall = anim_for(State::Falling { from_y: 0 });
        let back: Vec<_> = (0..8)
            .map(|t| cell_for(fall, ViewAngle::Back, t * TICKS_PER_FRAME))
            .collect();
        let want = [421, 422, 423, 422, 421, 420, 419, 420];
        assert_eq!(back, want.map(|c| (c, true)));
        assert_eq!(cell_for(fall, ViewAngle::Front, 0), (401, true));
        assert_eq!(
            cell_for(fall, ViewAngle::Front, 8 * TICKS_PER_FRAME),
            (401, true)
        );
        // Side views as stored, facing the way the lemming goes.
        assert_eq!(cell_for(fall, ViewAngle::Left, 0), (411, false));
        assert_eq!(cell_for(fall, ViewAngle::Right, 0), (411, true));
    }
}
