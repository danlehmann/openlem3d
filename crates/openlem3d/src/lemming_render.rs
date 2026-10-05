//! Turns simulation lemmings into camera-facing sprites from the `LEMM.MHC`
//! atlas.
//!
//! The cell ranges below are PROVISIONAL placeholders: which cells form which
//! action and viewing angle is being mapped from the running game and will
//! be recorded in `docs/spec/lemmings.md`.

use l3d_sim::{Lemming, SUB, State};

use crate::scene_build::{ATLAS_COLUMNS, LEMMING_CELL, LayerBuilder};
use crate::scene_render::SceneSprites;

/// Texels per world unit for lemming cells: a 64-texel cell spans half a
/// grid unit (estimated from relative sizes in the original's camera 1 view of
/// Fun 1; unverified).
const LEMMING_TEXELS_PER_UNIT: f32 = 128.0;

/// Simulation ticks per animation frame (provisional).
const TICKS_PER_FRAME: u32 = 2;

/// A run of consecutive cells forming one animation.
#[derive(Clone, Copy)]
struct Frames {
    first: u32,
    count: u32,
}

/// Provisional cell ranges per state.
fn frames_for(state: State) -> Frames {
    match state {
        State::Walking => Frames { first: 0, count: 10 },
        State::Falling { .. } => Frames { first: 128, count: 8 },
        State::Exiting => Frames { first: 392, count: 8 },
        State::Splatting => Frames { first: 160, count: 8 },
        State::Drowning => Frames { first: 440, count: 8 },
        State::Zapped => Frames { first: 512, count: 4 },
    }
}

/// Builds this frame's lemming sprites.
pub fn build(lemmings: &[Lemming], atlas_rows: u32) -> SceneSprites {
    let mut b = LayerBuilder::default();
    let tex = [(ATLAS_COLUMNS * LEMMING_CELL) as f32, (atlas_rows * LEMMING_CELL) as f32];
    for l in lemmings.iter().filter(|l| !l.gone) {
        let f = frames_for(l.state);
        let cell = f.first + (l.state_ticks / TICKS_PER_FRAME) % f.count;
        let (cx, cy) = ((cell % ATLAS_COLUMNS) * LEMMING_CELL, (cell / ATLAS_COLUMNS) * LEMMING_CELL);
        let rect = [cx as f32, cy as f32, LEMMING_CELL as f32, LEMMING_CELL as f32];
        let anchor = l.pos.map(|v| v as f32 / SUB as f32);
        b.sprite_scaled(anchor, rect, tex, 1, 0.0, LEMMING_TEXELS_PER_UNIT);
    }
    SceneSprites { vertices: b.vertices, indices: b.indices }
}
