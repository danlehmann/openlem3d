//! The in-level minimap in the panel's top-left corner, like the original's
//! (`docs/spec/ui-graphics.md`, "In-level panel"): a 64×64 top-down map,
//! two pixels per cell, in a wooden frame at (0, 0)–(70, 70), closed and
//! opened by the panel's red dot in its corner. Sea and land show their textures' average colour and each
//! column its highest block's top colour; the exit is red, hatches orange,
//! lemmings white and the camera a yellow dot (colours read off captures of
//! the original). Clicking it moves the camera over that spot, as the
//! original allows unless the level forbids it (flag `0x0008`).

use l3d_engine::prelude::*;
use l3d_formats::blk::FaceDir;
use l3d_formats::gamedata::Palette;
use l3d_formats::level::{SIZE_X, SIZE_Y, SIZE_Z};

use crate::World;
use crate::texture::rgba_image;
use crate::ui::NodeId;

/// Map pixels per grid cell.
const PX: usize = 2;
const MAP: usize = SIZE_X * PX;

/// Colours measured on the original's minimap (Practice levels).
const EXIT: [u8; 3] = [247, 0, 0];
const HATCH: [u8; 3] = [255, 162, 0];
const LEMMING: [u8; 3] = [255, 255, 255];
const CAMERA: [u8; 3] = [247, 231, 0];

/// Level header flags: minimap off, and click-to-place off.
const FLAG_NO_MINIMAP: u16 = 0x1000;
const FLAG_NO_MINIMAP_CLICK: u16 = 0x0008;

/// The map without moving dots, the image it is drawn into, and whether
/// clicks may move the camera.
pub struct Minimap {
    level: u32,
    base: Vec<[u8; 3]>,
    pub image: Handle<Image>,
    pub shown: bool,
    clickable: bool,
}

/// The pixels of the 70×70 frame: brown planks with a dark inner edge;
/// transparent inside. The panel's red dot sits in its top-left corner.
pub fn frame_image() -> Image {
    const N: usize = 70;
    let mut px = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let edge = x.min(y).min(N - 1 - x).min(N - 1 - y);
            let c: Option<[u8; 3]> = match edge {
                0 => Some([90, 52, 24]),
                1 => Some([150, 96, 48]),
                2 => Some([30, 20, 10]),
                _ => None,
            };
            if let Some([r, g, b]) = c {
                // A little grain along the planks.
                let grain = ((x * 7 + y * 13) % 5) as u8 * 6;
                let i = (y * N + x) * 4;
                px[i..i + 4].copy_from_slice(&[
                    r.saturating_add(grain),
                    g.saturating_add(grain / 2),
                    b,
                    255,
                ]);
            }
        }
    }
    rgba_image(N as u32, N as u32, px)
}

/// Average colour of the non-transparent pixels.
fn average(pixels: &[u8], pal: &Palette) -> [u8; 3] {
    let (mut sum, mut n) = ([0u32; 3], 0u32);
    for &p in pixels.iter().filter(|&&p| p != 0) {
        for (s, c) in sum.iter_mut().zip(pal[p as usize]) {
            *s += c as u32;
        }
        n += 1;
    }
    if n == 0 {
        [0, 0, 0]
    } else {
        sum.map(|s| (s / n) as u8)
    }
}

/// Builds the static map when a level has loaded.
pub(crate) fn build_base(w: &mut World) {
    let (current, game, data) = (&w.current, &w.game, &mut w.data);
    let images = &mut w.e.images;
    let existing = &w.minimap;
    let Some(n) = current.loaded else { return };
    if existing.as_ref().is_some_and(|m| m.level == n) {
        return;
    }
    let Some((level, blocks, _)) = &game.terrain else {
        return;
    };
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else {
        return;
    };
    let tex = d.gfx("TEXTURE", level.texture_set).unwrap_or_default();
    let tile = |t: u8| -> Option<[u8; 3]> {
        let s = t as usize * 64 * 64;
        tex.get(s..s + 64 * 64).map(|p| average(p, &pal))
    };
    let sea = (level.sea_gfx != 0xFF)
        .then(|| d.gfx("SEA", level.sea_gfx).ok())
        .flatten()
        .map_or([0, 70, 200], |s| {
            average(&s[..(64 * 64).min(s.len())], &pal)
        });
    let land = (level.land_gfx != 0xFF)
        .then(|| d.gfx("LAND", level.land_gfx).ok())
        .flatten()
        .map_or([40, 140, 40], |s| {
            average(&s[..(64 * 64).min(s.len())], &pal)
        });
    let polys: Vec<Vec<(f32, f32)>> = level
        .land_polygons
        .iter()
        .filter(|p| p.len() >= 3)
        .map(|p| p.iter().map(|v| (v.x as f32, v.z as f32)).collect())
        .collect();
    let on_land = |x: f32, z: f32| {
        polys.iter().any(|poly| {
            let mut sign = 0.0f32;
            (0..poly.len()).all(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                let c = (b.0 - a.0) * (z - a.1) - (b.1 - a.1) * (x - a.0);
                if c == 0.0 {
                    return true;
                }
                let ok = sign == 0.0 || c.signum() == sign;
                sign = c.signum();
                ok
            })
        })
    };
    let mut base = vec![sea; MAP * MAP];
    for z in 0..SIZE_Z {
        for x in 0..SIZE_X {
            let mut colour = if on_land(x as f32 + 0.5, z as f32 + 0.5) {
                land
            } else {
                sea
            };
            // A hatch or exit shows even under the blocks above it (Fun 16:
            // the hatch inside a tower).
            let mut column = (0..SIZE_Y).rev().map(|y| level.block(x, y, z));
            if let Some(b) = column.clone().find(|b| !b.is_empty() && b.id <= 1) {
                colour = if b.id == 0 { HATCH } else { EXIT };
            } else if let Some(b) = column.find(|b| !b.is_empty() && b.id != 3 && b.id != 4)
                && let Some(c) = blocks
                    .defs
                    .get(b.id as usize)
                    .and_then(|def| tile(def.face(FaceDir::PosY).texture))
            {
                colour = c;
            }
            for dz in 0..PX {
                for dx in 0..PX {
                    base[(z * PX + dz) * MAP + x * PX + dx] = colour;
                }
            }
        }
    }
    let image = rgba_image(MAP as u32, MAP as u32, vec![255; MAP * MAP * 4]);
    let handle = match &existing {
        Some(m) => {
            let _ = images.insert(&m.image, image);
            m.image.clone()
        }
        None => images.add(image),
    };
    w.minimap = Some(Minimap {
        level: n,
        base,
        image: handle,
        shown: level.flags & FLAG_NO_MINIMAP == 0,
        clickable: level.flags & FLAG_NO_MINIMAP_CLICK == 0,
    });
}

/// Redraws the map with the lemmings and the camera.
pub(crate) fn draw(w: &mut World) {
    let (Some(map), Some(sim)) = (&w.minimap, &w.game.sim) else {
        return;
    };
    let Some(image) = w.e.images.get_mut(&map.image) else {
        return;
    };
    let px = &mut image.data;
    for (i, c) in map.base.iter().enumerate() {
        px[i * 4..i * 4 + 3].copy_from_slice(c);
    }
    let mut dot = |x: f32, z: f32, c: [u8; 3]| {
        let (u, v) = ((x * PX as f32) as isize, (z * PX as f32) as isize);
        if (0..MAP as isize).contains(&u) && (0..MAP as isize).contains(&v) {
            let i = (v as usize * MAP + u as usize) * 4;
            px[i..i + 3].copy_from_slice(&c);
        }
    };
    for l in sim.lemmings.iter().filter(|l| !l.gone) {
        dot(crate::units(l.pos[0]), crate::units(l.pos[2]), LEMMING);
    }
    dot(w.view.pos.x, w.view.pos.z, CAMERA);
}

/// The minimap's on-screen node (the panel's).
#[derive(Default)]
pub struct MinimapView(pub Option<NodeId>);

/// A click on the map puts the camera over that spot.
pub(crate) fn click(w: &mut World) {
    if !w.minimap.as_ref().is_some_and(|m| m.clickable && m.shown)
        || !w.panel.is_open(crate::panel::Part::Minimap)
    {
        return;
    }
    let (mouse, touches) = (&w.e.input.mouse, &w.e.input.touches);
    let (window, screen) = (&w.e.window, &w.screen);
    let point = if mouse.just_pressed(MouseButton::Left) {
        screen.cursor(window)
    } else {
        touches
            .iter_just_released()
            .map(|t| screen.on_screen(t.position()))
            .next()
    };
    let node = w.minimap_view.0.and_then(|id| w.ui.get(id));
    let (Some(point), Some(node)) = (point, node) else {
        return;
    };
    let rect = node.computed.rect;
    let rel = (point - rect.min) / rect.size();
    let view = &mut w.view;
    if !(0.0..1.0).contains(&rel.x) || !(0.0..1.0).contains(&rel.y) {
        return;
    }
    // Keep the height and heading; stand a little back from the spot.
    let target = Vec3::new(rel.x * SIZE_X as f32, view.pos.y, rel.y * SIZE_Z as f32);
    let back = Vec3::new(-view.yaw.cos(), 0.0, -view.yaw.sin()) * 4.0;
    view.pos = target - back;
}
