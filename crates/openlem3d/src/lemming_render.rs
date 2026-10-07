//! Turns simulation lemmings into camera-facing sprites from the `LEMM.MHC`
//! atlas.
//!
//! Lemmings are pre-rendered from several angles; the sprite is chosen by
//! the lemming's heading relative to the camera ([`ViewAngle`]). Cell ranges
//! follow `docs/spec/lemmings.md` ("Frame groups"), where each is marked
//! verified or guessed.

use std::f32::consts::{FRAC_PI_4, TAU};

use l3d_sim::objects::ObjectKind;
use l3d_sim::{SUB, Simulation, State};

use crate::scene_build::{ATLAS_COLUMNS, BOMBNUMB_ATLAS_FIRST, DOOR_ATLAS_FIRST, DOOR_FRAMES, EXIT_ID, LEMMING_CELL, LayerBuilder, TRAP_ATLAS_CELLS, TRAP_ATLAS_FIRST, TRAP_FRAME};
use crate::scene_render::SceneSprites;

/// Texels per world unit for lemming cells: a cell spans half a grid unit
/// (estimated from relative sizes in the original's camera 1 view of Fun 1;
/// unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = LEMMING_CELL as f32 * 2.0;
/// The countdown digits (32 texels) stand a quarter unit tall.
const DIGIT_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame: one, at 14 ticks per second (the
/// original's sprites change once per tick; `docs/spec/behaviour.md`).
const TICKS_PER_FRAME: u32 = 1;

/// Ticks a bomber spends swelling before the blast.
const SWELL_TICKS: u32 = 10;

/// Ticks of each builder cycle spent stepping up onto the new brick.
const BUILDER_STEP_TICKS: u32 = 6;

/// The floater's umbrella cells.
const UMBRELLA_FIRST: u32 = 336;
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
    /// Eight blocks going round the lemming from the front (asymmetric poses).
    Eight,
    /// One block, the same from every side (smoke, sparks).
    One,
}

/// The order in which an action's frames are shown, one per step.
#[derive(Clone, Copy)]
enum Playback {
    /// Frames 0, 1, … in a loop.
    Loop,
    /// Frames 0, 1, …, then the last frame held.
    Once,
    /// The listed frame indices in a loop, every cell drawn mirrored relative
    /// to its angle block's usual orientation.
    MirroredCycle(&'static [u32]),
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
    Anim { first, frames, angles, playback: Playback::Loop }
}

const fn once(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim { first, frames, angles, playback: Playback::Once }
}

/// Falling frame order: a ping-pong over the five frames, starting in the
/// middle.
const FALL_CYCLE: &[u32] = &[2, 3, 4, 3, 2, 1, 0, 1];

/// The animation for a state (cell ranges from `docs/spec/lemmings.md`).
fn anim_for(state: State) -> Anim {
    use Angles::*;
    match state {
        State::Walking => anim(0, 6, Five),
        // Not found yet: lemmings walk into the exit.
        State::Exiting => anim(0, 6, Five),
        State::Turning { .. } => anim(30, 7, Eight),
        State::Blocking => anim(86, 7, Five),
        State::Falling { .. } => Anim { first: 399, frames: 5, angles: Five, playback: Playback::MirroredCycle(FALL_CYCLE) },
        State::Digging => anim(161, 8, Five),
        State::Bashing => anim(201, 8, Five),
        State::Mining => anim(241, 6, Five),
        // The puff of smoke left by the blast; the swelling before it is
        // drawn from the fuse (see `build`).
        State::Exploding => once(534, 4, One),
        State::Floating => anim(296, 5, Eight),
        // No rope pose found yet; the falling cycle stands in.
        // Arms spread on ice (observed); the blocker's pose is the nearest.
        State::Sliding => anim(86, 7, Five),
        // Hanging with arms raised (observed); the floater's pose stands in.
        State::OnRope { .. } => anim(296, 5, Eight),
        State::Flying { .. } | State::Bouncing { .. } => Anim { first: 399, frames: 5, angles: Five, playback: Playback::MirroredCycle(FALL_CYCLE) },
        State::Building { .. } => anim(344, 5, Five),
        State::Drowning => once(369, 6, Five),
        State::Splatting => once(429, 3, Five),
        State::Zapped | State::Trapped => anim(514, 2, Five),
        State::Climbing => anim(538, 5, Five),
        // Standing on the pad facing the camera (observed); see `build`.
        State::Teleporting { .. } => once(0, 1, One),
    }
}

/// The atlas cell and mirroring for an animation at a view angle and time.
fn cell_for(a: Anim, view: ViewAngle, ticks: u32) -> (u32, bool) {
    let step = ticks / TICKS_PER_FRAME;
    let (frame, flip) = match a.playback {
        Playback::Loop => (step % a.frames, false),
        Playback::Once => (step.min(a.frames - 1), false),
        Playback::MirroredCycle(cycle) => (cycle[step as usize % cycle.len()], true),
    };
    let (block, mirror) = match a.angles {
        Angles::Eight => (view.round_index(), false),
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
    (a.first + block * a.frames + frame, mirror != flip)
}

/// Interactive objects other than trampolines (`TRAPS` frames, one unit
/// tall). Bear traps and squashers rest on frame 0 and play frames 1–7
/// while busy; the weird trap rests on its last (empty) frame and plays
/// from 0; flame-blowers and lasers show only while firing. Springs show
/// frame 0. Which frames are idle is read off the sheets;
/// timings are provisional.
fn objects(b: &mut LayerBuilder, sim: &Simulation, tex: [f32; 2]) {
    const TRAP_TEXELS_PER_UNIT: f32 = 64.0;
    let Some(kind) = sim.object_kind else { return };
    let last = TRAP_ATLAS_CELLS - 1;
    for o in &sim.objects {
        let firing = o.busy > 0;
        // Progress through the busy time, 0 → frames − 1.
        let step = |frames: u32| ((l3d_sim::TRAP_BUSY_TICKS - o.busy) * frames / l3d_sim::TRAP_BUSY_TICKS).min(frames - 1);
        let frame = match kind {
            // Trampolines are pads in the static scene; rope-slide ends are not
            // drawn at all ([L3DEdit]).
            ObjectKind::Trampoline | ObjectKind::RopeSlide => continue,
            ObjectKind::BearTrap | ObjectKind::Squasher => if firing { 1 + step(last) } else { 0 },
            ObjectKind::WeirdTrap => if firing { step(TRAP_ATLAS_CELLS) } else { last },
            ObjectKind::FlameBlower | ObjectKind::Laser if !firing => continue,
            ObjectKind::FlameBlower | ObjectKind::Laser => o.busy % 4,
            // Teleporter pads are in the static scene too.
            ObjectKind::Teleporter => continue,
            ObjectKind::Spring => 0,
        };
        let cell = TRAP_ATLAS_FIRST + frame;
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let anchor = [o.cell[0] as f32 + 0.5, o.surface as f32 / SUB as f32, o.cell[2] as f32 + 0.5];
        let size = TRAP_FRAME as f32;
        b.sprite_scaled(anchor, [cx as f32, cy as f32, size, size], tex, 1, 0.0, TRAP_TEXELS_PER_UNIT);
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
    let shading = blocks.defs.get(EXIT_ID).map_or(0, |d| d.face(l3d_formats::blk::FaceDir::PosZ).shading);
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
        let face = [d.cell[0] as f32 + 0.5 + d.normal[0] as f32 * 0.5, d.cell[2] as f32 + 0.5 + d.normal[1] as f32 * 0.5];
        let entering = sim.lemmings.iter().any(|l| {
            let p = l.pos.map(|v| v as f32 / SUB as f32);
            !l.gone && l.state == State::Exiting && (p[0] - face[0]).abs() < 0.8 && (p[2] - face[1]).abs() < 0.8 && (p[1] - d.cell[1] as f32).abs() < 1.0
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
    use l3d_sim::{TELEPORT_APPEAR as APPEAR, TELEPORT_AWAY as AWAY, TELEPORT_STAND as STAND, TELEPORT_VANISH as VANISH};
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

/// Builds this frame's lemming sprites for a camera facing `camera_yaw`.
pub fn build(sim: &Simulation, atlas_rows: u32, camera_yaw: f32, doors: &[Door]) -> SceneSprites {
    let lemmings = &sim.lemmings;
    let mut b = LayerBuilder::default();
    let tex = [(ATLAS_COLUMNS * LEMMING_CELL) as f32, (atlas_rows * LEMMING_CELL) as f32];
    objects(&mut b, sim, tex);
    for d in doors.iter().filter(|d| d.frame > 0) {
        let cell = DOOR_ATLAS_FIRST + d.frame;
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let n = d.normal.map(|v| v as f32);
        let c = [d.cell[0] as f32 + 0.5 + n[0] * (0.5 + DOOR_OFFSET), d.cell[1] as f32, d.cell[2] as f32 + 0.5 + n[1] * (0.5 + DOOR_OFFSET)];
        b.vertical_quad(c, n, 1.0, 1.0, [cx as f32, cy as f32, 64.0, 64.0], tex, d.brightness);
    }
    // Trapped lemmings are shown by the trap's own animation.
    for l in lemmings.iter().filter(|l| !l.gone && l.state != State::Trapped) {
        let view = ViewAngle::from_yaws(l.dir.yaw(), camera_yaw);
        let (cell, mirror) = match l.state {
            // The stored turner points to its left (assumed from the one
            // verified case); one pointing right is its mirror image.
            State::Turning { to } if to == l.dir.clockwise() => {
                let (cell, mirror) = cell_for(anim_for(l.state), view.mirrored(), l.state_ticks);
                (cell, !mirror)
            }
            // Each brick: a few steps up with the sack (444–473), then the
            // laying motion spread over the rest of the 25-tick cycle
            // (provisional split).
            State::Building { .. } => {
                let c = l.state_ticks % l3d_sim::BUILD_TICKS;
                if c < BUILDER_STEP_TICKS {
                    cell_for(anim(444, 6, Angles::Five), view, c)
                } else {
                    let laying = (c - BUILDER_STEP_TICKS) * 5 / (l3d_sim::BUILD_TICKS - BUILDER_STEP_TICKS);
                    cell_for(once(344, 5, Angles::Five), view, laying)
                }
            }
            // A bomber swells over the last ticks of its fuse (observed:
            // about 10 ticks after the last countdown digit).
            _ if l.fuse.is_some_and(|f| f <= SWELL_TICKS) && !l.state.is_terminal() => {
                let swelled = SWELL_TICKS - l.fuse.unwrap_or(0);
                cell_for(once(271, 5, Angles::Five), view, swelled * 5 / SWELL_TICKS)
            }
            _ => cell_for(anim_for(l.state), view, l.state_ticks),
        };
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let size = LEMMING_CELL as f32;
        let rect = if mirror { [cx as f32 + size, cy as f32, -size, size] } else { [cx as f32, cy as f32, size, size] };
        let anchor = l.pos.map(|v| v as f32 / SUB as f32);
        // Teleporting: squeezed to a sliver, away, then stretched out again
        // (observed).
        let width = match l.state {
            State::Teleporting { to } => {
                if let Some(at) = sparkle_at(l.state_ticks, l.pos, to) {
                    let cell = TRAP_ATLAS_FIRST + SPARKLE_FIRST + l.state_ticks % SPARKLE_FRAMES;
                    let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
                    let s = TRAP_FRAME as f32;
                    b.sprite_scaled(at.map(|v| v as f32 / SUB as f32), [cx as f32, cy as f32, s, s], tex, 1, 0.0, SPARKLE_TEXELS_PER_UNIT);
                }
                teleport_width(l.state_ticks)
            }
            _ => 1.0,
        };
        // Exiting: shrinking into the doorway (observed), walking on into it.
        let (scale, anchor) = match l.state {
            State::Exiting => {
                let s = 1.0 - (l.state_ticks + 1).min(EXIT_SHRINK_TICKS) as f32 / EXIT_SHRINK_TICKS as f32;
                let [dx, _, dz] = l.dir.delta();
                let ahead = (1.0 - s) * EXIT_SHRINK_DEPTH;
                (s, [anchor[0] + dx as f32 * ahead, anchor[1], anchor[2] + dz as f32 * ahead])
            }
            _ => (1.0, anchor),
        };
        if width <= 0.0 || scale <= 0.0 {
            continue;
        }
        b.sprite_squeezed(anchor, rect, tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT / scale, width);
        // The bomber's countdown, 5…1, over its head (`BOMBNUMB` digits).
        if let Some(fuse) = l.fuse.filter(|_| !l.state.is_terminal()) {
            let digit = 5u32.saturating_sub((l3d_sim::FUSE_TICKS - fuse) / l3d_sim::FUSE_DIGIT_TICKS);
            if digit > 0 {
                let cell = BOMBNUMB_ATLAS_FIRST + digit - 1;
                let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
                let over = [anchor[0], anchor[1] + size / LEMMING_TEXELS_PER_UNIT, anchor[2]];
                b.sprite_scaled(over, [cx as f32, cy as f32, 32.0, 32.0], tex, 1, 0.0, DIGIT_TEXELS_PER_UNIT);
            }
        }
        if l.state == State::Floating {
            // The umbrella is stored apart (cells 336–343), its handle at the
            // bottom of the cell, held by the floater's raised hand at the top
            // of the floater's cell (frame order unverified).
            let cell = UMBRELLA_FIRST + l.state_ticks % UMBRELLA_FRAMES;
            let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
            let top = [anchor[0], anchor[1] + size / LEMMING_TEXELS_PER_UNIT, anchor[2]];
            b.sprite_scaled(top, [cx as f32, cy as f32, size, size], tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT);
        }
    }
    SceneSprites { vertices: b.vertices, indices: b.indices }
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
    fn walker_cells() {
        let walk = anim_for(State::Walking);
        assert_eq!(cell_for(walk, ViewAngle::Front, 0), (0, false));
        assert_eq!(cell_for(walk, ViewAngle::Right, 0), (12, true));
        assert_eq!(cell_for(walk, ViewAngle::Left, 0), (12, false));
        assert_eq!(cell_for(walk, ViewAngle::Back, 2 * TICKS_PER_FRAME), (26, false));
        assert_eq!(cell_for(walk, ViewAngle::FrontLeft, 0), (6, false));
    }

    #[test]
    fn faller_cells() {
        let fall = anim_for(State::Falling { from_y: 0 });
        let back: Vec<_> = (0..8).map(|t| cell_for(fall, ViewAngle::Back, t * TICKS_PER_FRAME)).collect();
        let want = [421, 422, 423, 422, 421, 420, 419, 420];
        assert_eq!(back, want.map(|c| (c, true)));
        assert_eq!(cell_for(fall, ViewAngle::Front, 0), (401, true));
        assert_eq!(cell_for(fall, ViewAngle::Front, 8 * TICKS_PER_FRAME), (401, true));
        // Left-hand angles: mirrored block, mirrored again.
        assert_eq!(cell_for(fall, ViewAngle::Left, 0), (411, true));
    }
}
