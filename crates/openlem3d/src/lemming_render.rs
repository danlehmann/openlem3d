//! Turns simulation lemmings into camera-facing sprites from the `LEMM.MHC`
//! atlas.
//!
//! Lemmings are pre-rendered from several angles; the sprite is chosen by
//! the lemming's heading relative to the camera ([`ViewAngle`]). Cell ranges
//! follow `docs/spec/lemmings.md` ("Frame groups"), where each is marked
//! verified or guessed.

use std::f32::consts::{FRAC_PI_4, TAU};

use l3d_sim::{Lemming, SUB, State};

use crate::scene_build::{ATLAS_COLUMNS, LEMMING_CELL, LayerBuilder};
use crate::scene_render::SceneSprites;

/// Texels per world unit for lemming cells: a 64-texel cell spans half a
/// grid unit (estimated from relative sizes in the original's camera 1 view of
/// Fun 1; unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame: about 15 frames per second at
/// 30 ticks per second (rough measurement, `docs/spec/lemmings.md`).
const TICKS_PER_FRAME: u32 = 2;

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
    /// Five blocks, front to back; the left-hand angles mirror the
    /// three-quarter and side blocks. The stored side views are assumed to
    /// face screen-right (unverified).
    Five,
    /// Eight blocks going round the lemming from the front (asymmetric poses).
    Eight,
}

/// One action's animation: `frames` cells per angle block starting at `first`.
#[derive(Clone, Copy)]
struct Anim {
    first: u32,
    frames: u32,
    angles: Angles,
    /// Hold the last frame instead of looping.
    once: bool,
}

const fn anim(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim { first, frames, angles, once: false }
}

const fn once(first: u32, frames: u32, angles: Angles) -> Anim {
    Anim { first, frames, angles, once: true }
}

/// The animation for a state (cell ranges from `docs/spec/lemmings.md`).
fn anim_for(state: State) -> Anim {
    use Angles::*;
    match state {
        State::Walking => anim(0, 6, Five),
        // Not found yet: lemmings walk into the exit.
        State::Exiting => anim(0, 6, Five),
        State::Turning => anim(30, 7, Eight),
        State::Blocking => anim(86, 7, Five),
        State::Falling { .. } => anim(121, 8, Five),
        State::Digging => anim(161, 8, Five),
        State::Bashing => anim(201, 8, Five),
        State::Mining => anim(241, 6, Five),
        State::Exploding => once(271, 5, Five),
        State::Floating => anim(296, 5, Eight),
        State::Building { .. } => anim(344, 5, Five),
        State::Drowning => once(369, 6, Five),
        State::Splatting => once(429, 3, Five),
        State::Zapped => anim(514, 2, Five),
        State::Climbing => anim(538, 5, Five),
    }
}

/// The atlas cell and mirroring for an animation at a view angle and time.
fn cell_for(a: Anim, view: ViewAngle, ticks: u32) -> (u32, bool) {
    let step = ticks / TICKS_PER_FRAME;
    let frame = if a.once { step.min(a.frames - 1) } else { step % a.frames };
    let (block, mirror) = match a.angles {
        Angles::Eight => (view.round_index(), false),
        Angles::Five => {
            let r = view.round_index();
            // Round positions 5–7 mirror 3–1.
            if r <= 4 { (r, false) } else { (8 - r, true) }
        }
    };
    (a.first + block * a.frames + frame, mirror)
}

/// Builds this frame's lemming sprites for a camera facing `camera_yaw`.
pub fn build(lemmings: &[Lemming], atlas_rows: u32, camera_yaw: f32) -> SceneSprites {
    let mut b = LayerBuilder::default();
    let tex = [(ATLAS_COLUMNS * LEMMING_CELL) as f32, (atlas_rows * LEMMING_CELL) as f32];
    for l in lemmings.iter().filter(|l| !l.gone) {
        let view = ViewAngle::from_yaws(l.dir.yaw(), camera_yaw);
        let (cell, mirror) = cell_for(anim_for(l.state), view, l.state_ticks);
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let size = LEMMING_CELL as f32;
        let rect = if mirror { [cx as f32 + size, cy as f32, -size, size] } else { [cx as f32, cy as f32, size, size] };
        let anchor = l.pos.map(|v| v as f32 / SUB as f32);
        b.sprite_scaled(anchor, rect, tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT);
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
        assert_eq!(cell_for(walk, ViewAngle::Right, 0), (12, false));
        assert_eq!(cell_for(walk, ViewAngle::Left, 0), (12, true));
        assert_eq!(cell_for(walk, ViewAngle::Back, 2 * TICKS_PER_FRAME), (26, false));
        assert_eq!(cell_for(walk, ViewAngle::FrontLeft, 0), (6, true));
    }
}
