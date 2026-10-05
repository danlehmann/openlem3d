//! Turns simulation lemmings into camera-facing sprites from the `LEMM.MHC`
//! atlas.
//!
//! Lemmings are pre-rendered from several angles; the sprite is chosen by
//! the lemming's heading relative to the camera ([`ViewAngle`]). The cell
//! ranges below are PROVISIONAL placeholders: which cells form which action
//! and angle is being mapped from the running game and is recorded in
//! `docs/spec/lemmings.md`.

use std::f32::consts::{FRAC_PI_4, TAU};

use l3d_sim::{Lemming, SUB, State};

use crate::scene_build::{ATLAS_COLUMNS, LEMMING_CELL, LayerBuilder};
use crate::scene_render::SceneSprites;

/// Texels per world unit for lemming cells: a 64-texel cell spans half a
/// grid unit (estimated from relative sizes in the original's camera 1 view of
/// Fun 1; unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame (provisional).
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
}

/// A run of consecutive cells forming one animation, optionally mirrored.
#[derive(Clone, Copy)]
struct Frames {
    first: u32,
    count: u32,
    mirror: bool,
}

const fn run(first: u32, count: u32) -> Frames {
    Frames { first, count, mirror: false }
}

/// Provisional cell ranges per state; `_view` is unused until the mapping
/// of angles to cells is known.
fn frames_for(state: State, _view: ViewAngle) -> Frames {
    match state {
        State::Walking => run(0, 10),
        State::Falling { .. } => run(128, 8),
        State::Floating => run(352, 8),
        State::Climbing => run(144, 8),
        State::Blocking => run(96, 8),
        State::Turning => run(112, 8),
        State::Digging => run(256, 8),
        State::Building { .. } => run(368, 8),
        State::Bashing => run(208, 8),
        State::Mining => run(232, 8),
        State::Exploding => run(528, 4),
        State::Exiting => run(392, 8),
        State::Splatting => run(160, 8),
        State::Drowning => run(440, 8),
        State::Zapped => run(512, 4),
    }
}

/// Builds this frame's lemming sprites for a camera facing `camera_yaw`.
pub fn build(lemmings: &[Lemming], atlas_rows: u32, camera_yaw: f32) -> SceneSprites {
    let mut b = LayerBuilder::default();
    let tex = [(ATLAS_COLUMNS * LEMMING_CELL) as f32, (atlas_rows * LEMMING_CELL) as f32];
    for l in lemmings.iter().filter(|l| !l.gone) {
        let view = ViewAngle::from_yaws(l.dir.yaw(), camera_yaw);
        let f = frames_for(l.state, view);
        let cell = f.first + (l.state_ticks / TICKS_PER_FRAME) % f.count;
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let size = LEMMING_CELL as f32;
        let rect = if f.mirror { [cx as f32 + size, cy as f32, -size, size] } else { [cx as f32, cy as f32, size, size] };
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
}
