//! The in-level minimap in the panel's top-left corner, like the original's
//! (`docs/spec/ui-graphics.md`, "In-level panel"): a 64×64 top-down map,
//! two pixels per cell, in a wooden frame at (0, 0)–(70, 70), closed and
//! opened by the panel's red dot in its corner. The map is centred on the
//! camera and turned with it, its view direction up. Sea and land show their
//! textures' first pixel (a level without a sea, or with the all-round sky,
//! its sky's first pixel),
//! each column the colour of its highest block's top height on a fixed
//! brown–green–grey–white ramp; the exit is red, hatches orange, lemmings
//! white and the camera a yellow dot (colours read off captures of the
//! original). Clicking it moves the camera over that spot, as the original
//! allows unless the level forbids it (flag `0x0008`).

use l3d_engine::prelude::*;
use l3d_formats::level::{SIZE_X, SIZE_Y, SIZE_Z};

use crate::World;
use crate::scene_build::level_flags::SURROUND_SKY;
use crate::texture::rgba_image;
use crate::ui::NodeId;

/// Map pixels per grid cell.
const PX: f32 = 2.0;
/// The map's side, pixels.
const MAP: usize = 64;
/// The map position (both axes, pixels) over the camera.
const CENTRE: f32 = MAP as f32 / 2.0;

/// `LM3D.PAL` indices of a column's colour by the height of its top, one
/// entry per half unit: entry `k` covers tops above `k / 2` units, up to
/// `(k + 1) / 2`, tops of ½ unit or less taking entry 0 too (measured on a
/// test level with tops at every quarter unit from ¼ to 16, 2026-11-03).
const HEIGHT_RAMP: [u8; 32] = [
    157, 157, 157, 156, 155, 154, 153, 152, 151, 150, 149, 116, 115, 114, 113, 112, 110, 109, 108,
    107, 106, 234, 232, 231, 230, 229, 227, 226, 225, 3, 2, 1,
];

/// Colours measured on the original's minimap (Practice levels).
const EXIT: [u8; 3] = [247, 0, 0];
const HATCH: [u8; 3] = [255, 162, 0];
const LEMMING: [u8; 3] = [255, 255, 255];
const CAMERA: [u8; 3] = [247, 231, 0];

/// Level header flags: minimap off, and click-to-place off.
const FLAG_NO_MINIMAP: u16 = 0x1000;
const FLAG_NO_MINIMAP_CLICK: u16 = 0x0008;

/// The map's colour per grid cell (`z * SIZE_X + x`) and beyond the grid,
/// the image it is drawn into, and whether clicks may move the camera.
pub struct Minimap {
    level: u32,
    cells: Vec<[u8; 3]>,
    background: [u8; 3],
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

/// Builds the static map when a level has loaded.
pub(crate) fn build_base(w: &mut World) {
    let (current, game, data) = (&w.current, &w.game, &mut w.data);
    let images = &mut w.e.images;
    let existing = &w.minimap;
    let Some(n) = current.loaded else { return };
    if existing.as_ref().is_some_and(|m| m.level == n) {
        return;
    }
    let Some((level, _, _)) = &game.terrain else {
        return;
    };
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else {
        return;
    };
    // Sea, land and sky each show the colour of their picture's first pixel
    // (measured to the RGB value: Practice sea 2 and land 15, Fun 1's sea
    // 1, a marked sea whose first pixel is red turning the whole map red).
    // Without a sea, or with the all-round sky (flag `0x0200`) even where
    // the level names one, the map shows the sky's (Tricky 15, Tricky 12,
    // Fun 8, Fun 1 with the flag set). Without either, unchecked.
    let mut first = |kind: &str, n: u8| {
        (n != 0xFF)
            .then(|| d.gfx(kind, n).ok())
            .flatten()
            .and_then(|s| s.first().map(|&p| pal[p as usize]))
            .map(|c| [c[0], c[1], c[2]])
    };
    let sky = first("SKY", level.sky_gfx).unwrap_or([0, 70, 200]);
    let sea = if level.flags & SURROUND_SKY != 0 {
        sky
    } else {
        first("SEA", level.sea_gfx).unwrap_or(sky)
    };
    let land = first("LAND", level.land_gfx).unwrap_or([40, 140, 40]);
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
    let mut cells = vec![sea; SIZE_X * SIZE_Z];
    for z in 0..SIZE_Z {
        for x in 0..SIZE_X {
            let colour = &mut cells[z * SIZE_X + x];
            if on_land(x as f32 + 0.5, z as f32 + 0.5) {
                *colour = land;
            }
            // A hatch or exit shows even under the blocks above it (Fun 16:
            // the hatch inside a tower).
            let mut column = (0..SIZE_Y)
                .rev()
                .map(|y| (y, level.block(x, y, z)))
                .filter(|(_, b)| !b.is_empty());
            if let Some((_, b)) = column.clone().find(|(_, b)| b.id <= 1) {
                *colour = if b.id == 0 { HATCH } else { EXIT };
            } else if let Some((y, b)) = column.find(|(_, b)| b.id != 3 && b.id != 4) {
                // The top in quarter units: above the cell's highest slice.
                let quarters = y * 4 + 8 - b.segments.leading_zeros() as usize;
                let entry = quarters.div_ceil(2).clamp(1, HEIGHT_RAMP.len()) - 1;
                let [r, g, b] = pal[HEIGHT_RAMP[entry] as usize];
                *colour = [r, g, b];
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
        cells,
        background: sea,
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
    let frame = MapFrame::of(&w.view);
    let px = &mut image.data;
    for v in 0..MAP {
        for u in 0..MAP {
            let (x, z) = frame.to_world(u as f32 + 0.5, v as f32 + 0.5);
            let inside = (0.0..SIZE_X as f32).contains(&x) && (0.0..SIZE_Z as f32).contains(&z);
            let c = if inside {
                map.cells[z as usize * SIZE_X + x as usize]
            } else {
                map.background
            };
            let i = (v * MAP + u) * 4;
            px[i..i + 3].copy_from_slice(&c);
        }
    }
    let mut dot = |x: f32, z: f32, c: [u8; 3]| {
        let (u, v) = frame.to_map(x, z);
        let (u, v) = (u.floor() as isize, v.floor() as isize);
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

/// Where the map lies over the level: [`CENTRE`] over the camera, its
/// up the camera's horizontal view direction.
struct MapFrame {
    /// The camera's `(x, z)`.
    centre: (f32, f32),
    /// The unit `(x, z)` direction the map's up shows.
    up: (f32, f32),
}

impl MapFrame {
    fn of(view: &crate::ViewCamera) -> Self {
        let f = view.forward();
        let len = f.x.hypot(f.z).max(1e-6);
        Self {
            centre: (view.pos.x, view.pos.z),
            up: (f.x / len, f.z / len),
        }
    }

    /// The level point `(x, z)` under map pixel position `(u, v)`.
    fn to_world(&self, u: f32, v: f32) -> (f32, f32) {
        let half = CENTRE;
        let (right, down) = ((u - half) / PX, (v - half) / PX);
        // The map's right is the up direction turned a quarter clockwise
        // seen from above: −Z up puts +X right.
        let (ux, uz) = self.up;
        (
            self.centre.0 - uz * right - ux * down,
            self.centre.1 + ux * right - uz * down,
        )
    }

    /// The map pixel position of the level point `(x, z)`.
    fn to_map(&self, x: f32, z: f32) -> (f32, f32) {
        let (dx, dz) = (x - self.centre.0, z - self.centre.1);
        let (ux, uz) = self.up;
        let half = CENTRE;
        let right = -uz * dx + ux * dz;
        let down = -(ux * dx + uz * dz);
        (half + right * PX, half + down * PX)
    }
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
    let (x, z) = MapFrame::of(view).to_world(rel.x * MAP as f32, rel.y * MAP as f32);
    let target = Vec3::new(x, view.pos.y, z);
    let back = Vec3::new(-view.yaw.cos(), 0.0, -view.yaw.sin()) * 4.0;
    view.pos = target - back;
}
