//! Light from glowing things falling on the blocks around them, in Enhanced
//! mode (ours, not the original's): torches, lamps and neon signs tint the
//! nearby faces that look towards them and that nothing solid hides from
//! them. Worked out per vertex when the block geometry is built.

use l3d_formats::gamedata::Palette;
use l3d_formats::level::{Level, SIZE_X, SIZE_Y, SIZE_Z};

/// A point light: where it is and its colour (linear RGB, at full
/// strength).
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: [f32; 3],
    pub colour: [f32; 3],
}

/// How far a light reaches, in grid units, and how strongly it lights a
/// face right next to it, facing it (ours, by eye).
const REACH: f32 = 3.0;
const STRENGTH: f32 = 0.9;
/// Steps per unit when checking whether something solid lies between a
/// light and a face.
const STEPS_PER_UNIT: f32 = 6.0;

/// The colour of the glowing pixels of `pixels`, weighted by their glow,
/// scaled so its strongest channel is 1; `None` if nothing glows.
pub fn glow_colour(pixels: &[u8], glow: &[f32], pal: &Palette) -> Option<[f32; 3]> {
    let mut sum = [0.0f32; 3];
    for (&i, &g) in pixels.iter().zip(glow) {
        for (s, c) in sum.iter_mut().zip(pal[i as usize]) {
            *s += g * linear(c);
        }
    }
    let m = sum.iter().cloned().fold(0.0, f32::max);
    (m > 0.0).then(|| sum.map(|s| s / m))
}

fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Whether the cell holding `p` has a block in it.
fn solid(level: &Level, p: [f32; 3]) -> bool {
    let [x, y, z] = p.map(|c| c.floor());
    if x < 0.0 || y < 0.0 || z < 0.0 {
        return false;
    }
    let (x, y, z) = (x as usize, y as usize, z as usize);
    x < SIZE_X && y < SIZE_Y && z < SIZE_Z && !level.block(x, y, z).is_empty()
}

/// The light reaching a face at `pos` with outward `normal` (linear RGB,
/// added to the face's own shading).
pub fn shine(lights: &[Light], level: &Level, pos: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let mut total = [0.0; 3];
    // Start just off the face, so its own block does not hide it.
    let from = [0, 1, 2].map(|i| pos[i] + normal[i] * 0.05);
    for l in lights {
        let d = [0, 1, 2].map(|i| l.pos[i] - from[i]);
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if dist >= REACH || dist < 1e-4 {
            continue;
        }
        let facing = (d[0] * normal[0] + d[1] * normal[1] + d[2] * normal[2]) / dist;
        // Wrapped a little, so faces side-on to a light still catch some.
        let facing = ((facing + 0.25) / 1.25).max(0.0);
        if facing == 0.0 {
            continue;
        }
        let steps = (dist * STEPS_PER_UNIT) as usize;
        let hidden = (1..steps).any(|k| {
            solid(
                level,
                [0, 1, 2].map(|i| from[i] + d[i] * k as f32 / steps as f32),
            )
        });
        if hidden {
            continue;
        }
        let falloff = (1.0 - dist / REACH).powi(2);
        for (t, c) in total.iter_mut().zip(l.colour) {
            *t += c * STRENGTH * falloff * facing;
        }
    }
    total
}
