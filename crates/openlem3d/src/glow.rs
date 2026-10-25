//! Which pixels glow in Enhanced mode (ours, not the original's): flames,
//! lamps, neon signs, sparkles, laser beams, electrified floors (the
//! current crackling over them) and the glints on ice are drawn brighter than
//! white, and the scene renderer lets that excess bloom into their
//! surroundings. Everything else keeps its palette colour exactly.
//!
//! A texel's glow (0–1) travels in its alpha channel: 255 is plain, lower
//! values glow, down to `255 − GLOW_STEPS` at full strength. That stays
//! above the cutout threshold, so glowing texels are never see-through.

use l3d_formats::blk::{BlockSet, flags};
use l3d_formats::gamedata::Palette;
use l3d_formats::level::Level;

/// Alpha steps between a plain texel and a fully glowing one; the scene
/// shader decodes the same scale.
pub const GLOW_STEPS: f32 = 120.0;

/// `ANIMOBJ` sets that are light sources: torches, lamps, the ring of fire,
/// the neon exit signs and the hanging lamp (judged by eye from the data).
const GLOWING_ANIM_OBJECTS: [u8; 8] = [0, 2, 3, 7, 8, 9, 11, 22];

/// Trap sheets with something glowing in them, and which colours glow:
/// the flame-thrower's fire (warm), the teleporter's sparkle and the laser
/// beams (cool).
const FIRE_TRAP: u8 = 0;
const SPARKLE_TRAP: u8 = 5;
const LASER_TRAP: u8 = 7;

/// The alpha for a texel of palette index `i` with glow `g`.
pub fn alpha(i: u8, g: f32) -> u8 {
    if i == 0 {
        0
    } else {
        255 - (g.clamp(0.0, 1.0) * GLOW_STEPS).round() as u8
    }
}

/// How strongly a colour glows when it is part of a light source: bright
/// colours fully, dark ones not at all, so a flame's hot core outshines its
/// red fringe.
fn strength(rgb: [u8; 3]) -> f32 {
    let v = *rgb.iter().max().unwrap() as f32 / 255.0;
    let t = ((v - 0.45) / 0.55).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whether palette index `i` is in the warm ramps (yellow to red) or is one
/// of the near-whites at the top of the palette.
fn warm(i: u8) -> bool {
    (1..=3).contains(&i) || (32..96).contains(&i)
}

/// Whether palette index `i` is in the white-to-blue ramps.
fn cool(i: u8) -> bool {
    (1..32).contains(&i)
}

/// The glow of palette index `i` in trap sheet `trap`.
pub fn trap(trap: u8, i: u8, pal: &Palette) -> f32 {
    let lit = match trap {
        FIRE_TRAP => warm(i),
        SPARKLE_TRAP | LASER_TRAP => cool(i),
        _ => false,
    };
    if lit { strength(pal[i as usize]) } else { 0.0 }
}

/// Wall torches in the block textures: tiles by texture set (found in the
/// data by their flame colours, checked by eye).
const TORCH_TILES: [(u8, &[usize]); 5] = [
    (
        0,
        &[
            12, 13, 14, 15, 16, 17, 18, 19, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
        ],
    ),
    (1, &[12, 13, 14, 15, 16, 17, 18, 19, 28, 29, 30, 31]),
    (10, &[28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39]),
    (21, &[12, 13, 14, 15, 16, 17, 18, 19, 28, 29, 30, 31]),
    (38, &[12, 13, 14, 15, 16, 17, 18, 19]),
];

/// Whether tile `tile` of texture set `set` shows a wall torch.
pub fn torch_tile(set: u8, tile: usize) -> bool {
    TORCH_TILES
        .iter()
        .any(|(s, tiles)| *s == set && tiles.contains(&tile))
}

/// Per-pixel glow of texture set `set`, a strip of 64×64 tiles: the flames
/// of its wall torches.
pub fn texture(set: u8, pixels: &[u8], pal: &Palette) -> Vec<f32> {
    pixels
        .iter()
        .enumerate()
        .map(|(p, &i)| {
            if torch_tile(set, p / (64 * 64)) && warm(i) {
                strength(pal[i as usize])
            } else {
                0.0
            }
        })
        .collect()
}

/// Level header flag: its liquid electrocutes rather than drowns.
const LIQUID_ZAPS: u16 = 0x8000;

/// The texture animations (first tile, frame count as in
/// `level_mesh::face_animation`) on the faces of liquid blocks, on a level
/// whose liquid electrocutes: its current.
pub fn electric_runs(level: &Level, blocks: &BlockSet) -> Vec<(u32, i32)> {
    if level.flags & LIQUID_ZAPS == 0 {
        return Vec::new();
    }
    let mut runs: Vec<(u32, i32)> = blocks
        .defs
        .iter()
        .filter(|d| d.flags & flags::LIQUID != 0 && !d.is_placeholder())
        .flat_map(|d| d.faces)
        .filter_map(|f| crate::level_mesh::face_animation(f.texture, f.modifiers))
        .collect();
    runs.sort_unstable();
    runs.dedup();
    runs
}

/// Per-pixel glow of the texture strip `pixels` (64×64 tiles) from the
/// animations `runs`: in each frame, the pixels that differ from what that
/// spot shows in most frames. The sparks run all over the grating from frame
/// to frame, so the grating is what most frames show, and only the current
/// glows.
pub fn electric(pixels: &[u8], runs: &[(u32, i32)], pal: &Palette) -> Vec<f32> {
    const TILE: usize = 64 * 64;
    let mut glow = vec![0.0; pixels.len()];
    for &(first, frames) in runs {
        let tiles: Vec<usize> = (0..frames.unsigned_abs() as usize)
            .map(|f| first as usize + f)
            .filter(|t| (t + 1) * TILE <= pixels.len())
            .collect();
        for p in 0..TILE {
            let at = |t: usize| pixels[t * TILE + p];
            let usual = tiles
                .iter()
                .map(|&t| at(t))
                .max_by_key(|&i| tiles.iter().filter(|&&t| at(t) == i).count());
            for &t in &tiles {
                let i = at(t);
                if i != 0 && Some(i) != usual {
                    glow[t * TILE + p] = strength(pal[i as usize]);
                }
            }
        }
    }
    glow
}

/// The tiles on the faces of slippery (ice) blocks.
pub fn ice_tiles(blocks: &BlockSet) -> Vec<usize> {
    let mut tiles: Vec<usize> = blocks
        .defs
        .iter()
        .filter(|d| d.flags & flags::SLIPPERY != 0 && !d.is_placeholder())
        .flat_map(|d| d.faces)
        .filter(|f| f.texture != 0xFF)
        .map(|f| f.texture as usize)
        .collect();
    tiles.sort_unstable();
    tiles.dedup();
    tiles
}

/// Per-pixel glow of the texture strip `pixels` (64×64 tiles) on `tiles`:
/// the ice's glints, its texels clearly brighter than those around them,
/// as [`stars`] picks out a sky's stars.
pub fn ice(pixels: &[u8], tiles: &[usize], pal: &Palette) -> Vec<f32> {
    const TILE: usize = 64 * 64;
    const R: isize = 2;
    let mut glow = vec![0.0; pixels.len()];
    for &t in tiles.iter().filter(|&&t| (t + 1) * TILE <= pixels.len()) {
        let tile = &pixels[t * TILE..(t + 1) * TILE];
        let value = |x: isize, y: isize| {
            let rgb = pal[tile[(y.rem_euclid(64) * 64 + x.rem_euclid(64)) as usize] as usize];
            *rgb.iter().max().unwrap() as f32 / 255.0
        };
        for p in 0..TILE {
            let (x, y) = ((p % 64) as isize, (p / 64) as isize);
            let mut sum = 0.0;
            for dy in -R..=R {
                for dx in -R..=R {
                    sum += value(x + dx, y + dy);
                }
            }
            let around = sum / ((2 * R + 1) * (2 * R + 1)) as f32;
            let v = value(x, y);
            glow[t * TILE + p] =
                ((v - around - 0.08) / 0.2).clamp(0.0, 1.0) * ((v - 0.6) / 0.3).clamp(0.0, 1.0);
        }
    }
    glow
}

/// Per-pixel glow of an all-round (space) sky, `width` wide and wrapping
/// across: its stars, the pixels clearly brighter than their surroundings,
/// rather than the bright but smooth nebulae.
pub fn stars(pixels: &[u8], width: usize, pal: &Palette) -> Vec<f32> {
    const R: isize = 3;
    let height = pixels.len() / width;
    let value = |x: isize, y: isize| {
        let x = x.rem_euclid(width as isize) as usize;
        let y = y.clamp(0, height as isize - 1) as usize;
        let rgb = pal[pixels[y * width + x] as usize];
        *rgb.iter().max().unwrap() as f32 / 255.0
    };
    (0..pixels.len())
        .map(|p| {
            let (x, y) = ((p % width) as isize, (p / width) as isize);
            let mut sum = 0.0;
            for dy in -R..=R {
                for dx in -R..=R {
                    sum += value(x + dx, y + dy);
                }
            }
            let around = sum / ((2 * R + 1) * (2 * R + 1)) as f32;
            ((value(x, y) - around - 0.12) / 0.3).clamp(0.0, 1.0)
        })
        .collect()
}

/// Lemming sprite cells (`LEMM.MHC`) that light up: the electrocuted
/// lemming's X-ray skeleton (the odd cells of 514–523, alternating with the
/// lemming) and the lightning under the zap cloud (524–533, its yellow).
/// The smoke puff after (534–537) stays grey, as in the original.
pub fn lemming(cell: u32, i: u8, pal: &Palette) -> f32 {
    let lit = match cell {
        515..=523 if cell % 2 == 1 && cool(i) => 1.0,
        524..=533 if (43..=45).contains(&i) => 1.0,
        _ => 0.0,
    };
    lit * strength(pal[i as usize])
}

/// Per-pixel glow of `ANIMOBJ` set `set`, a 64-wide strip of 64×64 frames.
/// Only what changes between frames glows (the flame, not the torch holding
/// it), widened by a pixel so a flame's steady middle glows too.
pub fn anim_object(set: u8, pixels: &[u8], pal: &Palette) -> Vec<f32> {
    let mut glow = vec![0.0; pixels.len()];
    if !GLOWING_ANIM_OBJECTS.contains(&set) {
        return glow;
    }
    const W: usize = 64;
    const FRAME: usize = 64 * 64;
    let frames = pixels.len() / FRAME;
    let changes: Vec<bool> = (0..FRAME)
        .map(|p| (1..frames).any(|f| pixels[f * FRAME + p] != pixels[p]))
        .collect();
    let near_change = |p: usize| {
        let (x, y) = ((p % W) as isize, (p / W) as isize);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let (nx, ny) = (x + dx, y + dy);
                (0..W as isize).contains(&nx)
                    && (0..W as isize).contains(&ny)
                    && changes[ny as usize * W + nx as usize]
            })
        })
    };
    let near: Vec<bool> = (0..FRAME).map(near_change).collect();
    for (p, g) in glow.iter_mut().enumerate().take(frames * FRAME) {
        let i = pixels[p];
        if i != 0 && near[p % FRAME] {
            *g = strength(pal[i as usize]);
        }
    }
    glow
}
