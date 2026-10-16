//! Which pixels glow in Enhanced mode (ours, not the original's): flames,
//! lamps, neon signs, sparkles and laser beams are drawn brighter than
//! white, and the scene renderer lets that excess bloom into their
//! surroundings. Everything else keeps its palette colour exactly.
//!
//! A texel's glow (0–1) travels in its alpha channel: 255 is plain, lower
//! values glow, down to `255 − GLOW_STEPS` at full strength. That stays
//! above the cutout threshold, so glowing texels are never see-through.

use l3d_formats::gamedata::Palette;

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
/// lemming), the lightning under the zap cloud (524–533, its yellow) and the
/// start of a bomber's explosion (534 and, dimmer, 535), drawn as smoke but
/// flashing white-hot here.
pub fn lemming(cell: u32, i: u8, pal: &Palette) -> f32 {
    let lit = match cell {
        515..=523 if cell % 2 == 1 && cool(i) => 1.0,
        524..=533 if (43..=45).contains(&i) => 1.0,
        534 => 1.0,
        535 => 0.4,
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
