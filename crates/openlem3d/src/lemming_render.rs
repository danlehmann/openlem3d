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

use crate::scene_build::{
    ATLAS_COLUMNS, BOMBNUMB_ATLAS_FIRST, CRACK_ATLAS_FIRST, CRACK_FRAMES, DOOR_ATLAS_FIRST, DOOR_FRAMES, EXIT_ID, LEMMING_CELL, LayerBuilder, TILE_ATLAS_FIRST, TRAP_ATLAS_CELLS, TRAP_ATLAS_FIRST,
    TRAP_FRAME,
};
use crate::scene_render::SceneSprites;
use l3d_formats::blk::{BlockSet, FaceDir};

/// Texels per world unit for lemming cells: a cell spans half a grid unit
/// (estimated from relative sizes in the original's camera 1 view of Fun 1;
/// unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = LEMMING_CELL as f32 * 2.0;
/// The countdown digits (32 texels) stand a quarter unit tall.
const DIGIT_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame: one, at 14 ticks per second (the
/// original's sprites change once per tick; `docs/spec/behaviour.md`).
const TICKS_PER_FRAME: u32 = 1;

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
    /// Eight blocks going round the lemming from the front by its right
    /// (asymmetric poses; the floater's order is unverified).
    Eight,
    /// Eight blocks going round the lemming from the front by its left, as
    /// the turner's: its block 2 (cells 44–50) faces screen-left and block 6
    /// (72–78) screen-right, the pointing arm towards the camera (verified
    /// by eye).
    EightByLeft,
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
    /// The listed frame indices in a loop, the front and back views drawn
    /// mirrored (verified for falls); the side views as stored, which face
    /// the way the lemming goes (mirrored, it fell looking backwards).
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
        State::Turning { .. } => anim(30, 7, EightByLeft),
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
        // Out of bricks: a shrug with the empty sack (cells 474–488).
        State::Shrugging => once(474, 3, Five),
        State::Drowning => once(369, 6, Five),
        State::Splatting => once(429, 3, Five),
        State::Zapped | State::Trapped => anim(514, 2, Five),
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
        Playback::Once => (step.min(a.frames - 1), false),
        Playback::MirroredCycle(cycle) => (cycle[step as usize % cycle.len()], true),
    };
    let (block, mirror) = match a.angles {
        Angles::Eight => (view.round_index(), false),
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
    (a.first + block * a.frames + frame, mirror != (flip && matches!(view, ViewAngle::Front | ViewAngle::Back)))
}

/// Interactive objects other than trampolines (`TRAPS` frames, one unit
/// tall). Bear traps and squashers rest on frame 0 and play frames 1–7
/// while busy; the weird trap rests on its last (empty) frame and plays
/// from 0; flame-blowers and lasers show only while firing. Which frames are idle is read off the sheets;
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
            // Trampolines are pads in the static scene; one a lemming has just
            // touched plays its dip (frames 1–3 of its colour) on top, flat
            // just above the static pad. Rope-slide ends are not drawn at
            // all ([L3DEdit]).
            ObjectKind::Trampoline => {
                if o.busy > 0 {
                    let colour = if o.value >= 0x64 { 4 } else { 0 };
                    let cell = TRAP_ATLAS_FIRST + colour + (l3d_sim::PAD_BUSY_TICKS - o.busy + 1).min(3);
                    let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
                    let y = o.surface as f32 / SUB as f32 + 0.008;
                    let (x0, z0) = (o.cell[0] as f32, o.cell[2] as f32);
                    let (x1, z1) = (x0 + 1.0, z0 + 1.0);
                    let s = TRAP_FRAME as f32;
                    b.quad([[x0, y, z1], [x1, y, z1], [x1, y, z0], [x0, y, z0]], [cx as f32, cy as f32, s, s], tex, 1.0);
                }
                continue;
            }
            ObjectKind::RopeSlide => continue,
            ObjectKind::BearTrap | ObjectKind::Squasher => if firing { 1 + step(last) } else { 0 },
            ObjectKind::WeirdTrap => if firing { step(TRAP_ATLAS_CELLS) } else { last },
            ObjectKind::FlameBlower | ObjectKind::Laser if !firing => continue,
            ObjectKind::FlameBlower | ObjectKind::Laser => o.busy % 4,
            // Teleporter and spring pads are in the static scene too; a
            // spring shows its launch (frames 1–7, side on) while busy.
            ObjectKind::Teleporter => continue,
            ObjectKind::Spring if o.busy == 0 => continue,
            ObjectKind::Spring => 1 + (l3d_sim::SPRING_BUSY_TICKS - o.busy).min(6),
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

/// The electrocution cloud's cells.
const ZAP_CLOUD_FIRST: u32 = 524;
const ZAP_CLOUD_FRAMES: u32 = 10;

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
        h = (h ^ k).wrapping_mul(0x85EB_CA6B).rotate_left(13).wrapping_mul(0xC2B2_AE35);
    }
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The bash effects (`docs/spec/behaviour.md`, Basher): the black cracks of
/// `OVERLAY.000` spreading over the sides of the segments being bashed, one
/// frame every 2 ticks, and bursts of pieces cut from the block's texture
/// flying out of the hole.
fn bash_effects(b: &mut LayerBuilder, sim: &Simulation, blocks: Option<&BlockSet>, tex: [f32; 2]) {
    for (i, l) in sim.lemmings.iter().enumerate().filter(|(_, l)| !l.gone) {
        let Some((cell, mask, t)) = sim.bash_target(l) else { continue };
        let c = cell.map(|v| v as f32);
        let crack = CRACK_ATLAS_FIRST + (t / 2).min(CRACK_FRAMES - 1);
        let (cx, cy) = ((crack % ATLAS_COLUMNS) * LEMMING_CELL, (crack / ATLAS_COLUMNS) * LEMMING_CELL);
        // Each segment's slice of the overlay: segment s (0 at the bottom) is
        // the face's texel rows (3 − s)·16 to (4 − s)·16.
        for n in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
            for s in (0..4).filter(|s| mask & (1 << s) != 0) {
                let centre = [c[0] + 0.5 + n[0] * (0.5 + CRACK_OFFSET), c[1] + s as f32 * 0.25, c[2] + 0.5 + n[1] * (0.5 + CRACK_OFFSET)];
                b.vertical_quad(centre, n, 1.0, 0.25, [cx as f32, cy as f32 + (3 - s) as f32 * 16.0, 64.0, 16.0], tex, 1.0);
            }
        }
        // The pieces: bits of the block's side texture.
        let id = sim.world.block(cell).map_or(0, |(b, _)| b.id as usize);
        let tile = blocks.and_then(|bs| bs.defs.get(id)).map_or(0xFF, |d| d.face(FaceDir::PosX).texture);
        if tile == 0xFF {
            continue;
        }
        let tile = tile as u32;
        let tile_cell = TILE_ATLAS_FIRST + tile / 4;
        let (tx, ty) = ((tile_cell % ATLAS_COLUMNS) * LEMMING_CELL + (tile % 2) * 64, (tile_cell / ATLAS_COLUMNS) * LEMMING_CELL + (tile / 2 % 2) * 64);
        let d = l.dir.delta().map(|v| v as f32);
        let (out, side) = ([-d[0], -d[2]], [d[2], -d[0]]);
        let face = [c[0] + 0.5 - d[0] * 0.5, c[2] + 0.5 - d[2] * 0.5];
        let bottom = c[1] + mask.trailing_zeros() as f32 * 0.25;
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
                let along = |a: usize| face[a] + out[a] * v_out * age + side[a] * (lateral + v_side * age);
                let y = y0 + v_up * age - CHUNK_GRAVITY * age * age / 2.0;
                let size = 6.0 + (r(5) * 5.0).floor();
                let (sx, sy) = ((r(6) * (64.0 - size)).floor(), (r(7) * (64.0 - size)).floor());
                b.sprite_scaled([along(0), y, along(1)], [tx as f32 + sx, ty as f32 + sy, size, size], tex, 1, 0.0, 96.0);
            }
        }
    }
}

/// A splatted lemming's fragments: texels of the blue smock (walker cell 0,
/// texels 22–40 across and 28–48 down) flying out on all sides and falling
/// back, over the death (`docs/spec/behaviour.md`, Splat: "a burst of blue
/// fragments", verified; count, speeds and spread are ours).
const SPLAT_PIECES: u32 = 40;
const SPLAT_SMOCK: [f32; 4] = [22.0, 28.0, 18.0, 20.0];
const SPLAT_OUT: (f32, f32) = (0.03, 0.09);
const SPLAT_UP: (f32, f32) = (0.04, 0.16);
const SPLAT_GRAVITY: f32 = 0.025;
/// Drawn larger than the lemming, so a piece is a few screen pixels.
const SPLAT_TEXELS_PER_UNIT: f32 = 64.0;

fn splat(b: &mut LayerBuilder, i: usize, t: u32, at: [f32; 3], tex: [f32; 2]) {
    let age = t as f32;
    for k in 0..SPLAT_PIECES {
        let r = |salt: u32| noise([i as u32, 0x5A1A7, k, salt, 0]);
        let angle = r(0) * std::f32::consts::TAU;
        let v_out = SPLAT_OUT.0 + r(1) * (SPLAT_OUT.1 - SPLAT_OUT.0);
        let v_up = SPLAT_UP.0 + r(2) * (SPLAT_UP.1 - SPLAT_UP.0);
        // Pieces stop at the ground they burst from.
        let y = (at[1] + 0.05 + v_up * age - SPLAT_GRAVITY * age * age / 2.0).max(at[1]);
        let p = [at[0] + angle.cos() * v_out * age, y, at[2] + angle.sin() * v_out * age];
        let size = 2.0 + (r(3) * 3.0).floor();
        let (sx, sy) = ((r(4) * (SPLAT_SMOCK[2] - size)).floor(), (r(5) * (SPLAT_SMOCK[3] - size)).floor());
        b.sprite_scaled(p, [SPLAT_SMOCK[0] + sx, SPLAT_SMOCK[1] + sy, size, size], tex, 1, 0.0, SPLAT_TEXELS_PER_UNIT);
    }
}

/// Builds this frame's lemming sprites for a camera facing `camera_yaw`,
/// leaving out lemming `eyes`, the one the view looks out of, and marking
/// lemming `highlight` with an arrow.
#[allow(clippy::too_many_arguments)]
pub fn build(sim: &Simulation, blocks: Option<&BlockSet>, atlas_rows: u32, camera_yaw: f32, doors: &[Door], eyes: Option<usize>, highlight: Option<usize>) -> SceneSprites {
    let lemmings = &sim.lemmings;
    let mut b = LayerBuilder::default();
    let tex = [(ATLAS_COLUMNS * LEMMING_CELL) as f32, (atlas_rows * LEMMING_CELL) as f32];
    objects(&mut b, sim, tex);
    bash_effects(&mut b, sim, blocks, tex);
    for d in doors.iter().filter(|d| d.frame > 0) {
        let cell = DOOR_ATLAS_FIRST + d.frame;
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let n = d.normal.map(|v| v as f32);
        let c = [d.cell[0] as f32 + 0.5 + n[0] * (0.5 + DOOR_OFFSET), d.cell[1] as f32, d.cell[2] as f32 + 0.5 + n[1] * (0.5 + DOOR_OFFSET)];
        b.vertical_quad(c, n, 1.0, 1.0, [cx as f32, cy as f32, 64.0, 64.0], tex, d.brightness);
    }
    // Trapped lemmings are shown by the trap's own animation.
    for (i, l) in lemmings.iter().enumerate().filter(|(i, l)| !l.gone && l.state != State::Trapped && Some(*i) != eyes) {
        // A turner stands facing the walkers coming up behind it, the way it
        // came (seen in the original's "Turner" demo; drawn the other way,
        // turners showed their backs to the cameras of "Take a Dive").
        // A blocker stands side-on, its outstretched arms pointing back the
        // way it came and on the way it was going (owner's observation).
        let facing = match l.state {
            State::Turning { .. } => l.dir.reverse(),
            State::Blocking => l.dir.clockwise(),
            _ => l.dir,
        };
        let view = ViewAngle::from_yaws(facing.yaw(), camera_yaw);
        let (cell, mirror) = match l.state {
            // The stored turner points to its right (cells 30–36, seen from
            // the front, hold out the lemming's right arm); one pointing
            // left is its mirror image. The reverse showed turners pointing
            // away from where they sent walkers.
            State::Turning { to } if to == facing.anticlockwise() => {
                let (cell, mirror) = cell_for(anim_for(l.state), view.mirrored(), l.state_ticks);
                (cell, !mirror)
            }
            // Each brick: a few steps up with the sack (444–473), then the
            // laying motion spread over the rest of the 25-tick cycle
            // (provisional split).
            State::Building { .. } => {
                // In step with the bricks, the first of which comes
                // FIRST_BRICK_TICKS in, after a laying motion.
                let c = (l.state_ticks + l3d_sim::BUILD_TICKS - l3d_sim::FIRST_BRICK_TICKS) % l3d_sim::BUILD_TICKS;
                if c < BUILDER_STEP_TICKS {
                    cell_for(anim(444, 6, Angles::Five), view, c)
                } else {
                    let laying = (c - BUILDER_STEP_TICKS) * 5 / (l3d_sim::BUILD_TICKS - BUILDER_STEP_TICKS);
                    cell_for(once(344, 5, Angles::Five), view, laying)
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
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let size = LEMMING_CELL as f32;
        let rect = if mirror { [cx as f32 + size, cy as f32, -size, size] } else { [cx as f32, cy as f32, size, size] };
        let anchor = l.pos.map(|v| v as f32 / SUB as f32);
        if l.state == State::Splatting {
            splat(&mut b, i, l.state_ticks, anchor, tex);
            continue;
        }
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
        if Some(i) == highlight {
            // The highlight arrow over the lemming (`BOMBNUMB` cell 6).
            let cell = BOMBNUMB_ATLAS_FIRST + 6;
            let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
            let over = [anchor[0], anchor[1] + size / LEMMING_TEXELS_PER_UNIT, anchor[2]];
            b.sprite_scaled(over, [cx as f32, cy as f32, 32.0, 32.0], tex, 1, 0.0, DIGIT_TEXELS_PER_UNIT);
        }
        if l.state == State::Zapped {
            // The electrocution cloud over the lemming: it gathers, strikes
            // with lightning and shrinks away (cells 524–533, view-independent;
            // spread over the death, timing unmeasured).
            let frame = (l.state_ticks * ZAP_CLOUD_FRAMES / l3d_sim::DEATH_TICKS).min(ZAP_CLOUD_FRAMES - 1);
            let cell = ZAP_CLOUD_FIRST + frame;
            let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
            let over = [anchor[0], anchor[1] + size / LEMMING_TEXELS_PER_UNIT * 0.6, anchor[2]];
            b.sprite_scaled(over, [cx as f32, cy as f32, size, size], tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT);
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
        // Side views as stored, facing the way the lemming goes.
        assert_eq!(cell_for(fall, ViewAngle::Left, 0), (411, false));
        assert_eq!(cell_for(fall, ViewAngle::Right, 0), (411, true));
    }
}
