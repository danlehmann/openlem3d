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
//!
//! In enhanced mode (ours) the same area is drawn at four times the
//! resolution from the simulation's terrain, sampled eight times per cell
//! along each side: each top in its tile's average colour, shaded by height
//! and slope, edged where it steps, kept up to date as lemmings dig, bash and
//! build; hatches and exits as framed squares, objects as small squares, the
//! camera as a view cone and lemmings as dots coloured by what they do.

use std::f32::consts::FRAC_1_SQRT_2;

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

/// Enhanced mode (ours): image pixels per pixel of the original's map, the
/// same area drawn sharper in the same place.
const HI: usize = 4;
/// Enhanced mode: height samples per grid cell along each axis.
const SAMPLES: usize = 8;
/// Enhanced mode: samples along each axis of the whole grid.
const GRID: usize = SIZE_X * SAMPLES;
/// Enhanced mode: the camera's view cone, its half angle (radians, about
/// half the horizontal field of view) and length (image pixels).
const CONE_HALF: f32 = 0.55;
const CONE_LEN: f32 = 48.0;
/// Enhanced mode: lemming colours by what they are doing.
const BLOCKER: [u8; 3] = [255, 70, 70];
const TURNER: [u8; 3] = [80, 230, 255];
const BUILDER: [u8; 3] = [255, 160, 40];
const DIGGING: [u8; 3] = [255, 230, 60];
const CLIMBING: [u8; 3] = [150, 255, 120];
/// Enhanced mode: killing traps, and other objects (springs, trampolines,
/// teleporters, rope slides).
const TRAP: [u8; 3] = [230, 60, 230];
const OBJECT: [u8; 3] = [60, 200, 255];

/// The map's colour per grid cell (`z * SIZE_X + x`) and beyond the grid,
/// the image it is drawn into, and whether clicks may move the camera.
pub struct Minimap {
    level: u32,
    cells: Vec<[u8; 3]>,
    background: [u8; 3],
    pub image: Handle<Image>,
    pub shown: bool,
    clickable: bool,
    /// Enhanced mode: sea or land per grid cell, as under any block.
    ground: Vec<[u8; 3]>,
    /// Enhanced mode: the average colour of each block id's top tile.
    top_colour: Vec<[u8; 3]>,
    /// Enhanced mode: hatch and exit cells `(x, z)`, with their colours.
    marks: Vec<((usize, usize), [u8; 3])>,
    /// Enhanced mode: the terrain sampled from the simulation's world.
    detail: Option<Detail>,
}

/// Enhanced mode: the terrain's top, [`SAMPLES`] × [`SAMPLES`] samples per
/// grid cell, indexed `(z * SAMPLES + sz) * GRID + x * SAMPLES + sx`.
struct Detail {
    /// The top's height in units; `NAN` where nothing visible stands.
    height: Vec<f32>,
    /// The top's colour.
    colour: Vec<[u8; 3]>,
    /// Per grid column, a digest of its blocks and bricks when sampled.
    digest: Vec<u64>,
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
    let Some((level, blocks, _)) = &game.terrain else {
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
    // Enhanced mode: each block's top tile, averaged (palette index 0, the
    // see-through colour, left out).
    let tiles = d.gfx("TEXTURE", level.texture_set).ok();
    let top_colour: Vec<[u8; 3]> = blocks
        .defs
        .iter()
        .map(|def| {
            let tile = def.faces[4].texture as usize;
            let texels = tiles
                .as_ref()
                .and_then(|t| t.get(tile * 64 * 64..(tile + 1) * 64 * 64));
            let mut sum = [0u32; 3];
            let mut n = 0;
            for &p in texels.into_iter().flatten().filter(|&&p| p != 0) {
                let c = pal[p as usize];
                (0..3).for_each(|i| sum[i] += c[i] as u32);
                n += 1;
            }
            if n == 0 {
                [150, 150, 150]
            } else {
                sum.map(|s| (s / n) as u8)
            }
        })
        .collect();
    let mut cells = vec![sea; SIZE_X * SIZE_Z];
    let mut ground = vec![sea; SIZE_X * SIZE_Z];
    let mut marks = Vec::new();
    for z in 0..SIZE_Z {
        for x in 0..SIZE_X {
            let colour = &mut cells[z * SIZE_X + x];
            if on_land(x as f32 + 0.5, z as f32 + 0.5) {
                *colour = land;
                ground[z * SIZE_X + x] = land;
            }
            // A hatch or exit shows even under the blocks above it (Fun 16:
            // the hatch inside a tower).
            let mut column = (0..SIZE_Y)
                .rev()
                .map(|y| (y, level.block(x, y, z)))
                .filter(|(_, b)| !b.is_empty());
            if let Some((_, b)) = column.clone().find(|(_, b)| b.id <= 1) {
                *colour = if b.id == 0 { HATCH } else { EXIT };
                marks.push(((x, z), *colour));
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
        ground,
        top_colour,
        marks,
        detail: None,
    });
}

/// Enhanced mode: a digest of grid column `(x, z)`'s blocks (with their
/// cuts) and the bricks over it, to tell when it needs sampling again.
fn column_digest(world: &l3d_sim::World, x: i32, z: i32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for y in 0..SIZE_Y as i32 {
        let c = [x, y, z];
        world
            .block(c)
            .map(|(b, _)| (b.id, b.shape, b.rotation, b.segments))
            .hash(&mut h);
        (world.cut(c), world.base(c)).hash(&mut h);
    }
    let (x0, z0) = (x * l3d_sim::SUB, z * l3d_sim::SUB);
    for b in &world.bricks {
        if b.min[0] < x0 + l3d_sim::SUB
            && x0 < b.max[0]
            && b.min[2] < z0 + l3d_sim::SUB
            && z0 < b.max[2]
        {
            (b.min, b.max, b.id).hash(&mut h);
        }
    }
    h.finish()
}

/// Enhanced mode: samples grid column `(x, z)`'s top into `detail`: at
/// each sample, the highest visible solid point (sixteenth-unit steps) and
/// its block's top colour; invisible blocks (ids 3 and 4) are looked through.
fn sample_column(map: &Minimap, world: &l3d_sim::World, x: usize, z: usize, detail: &mut Detail) {
    use l3d_sim::SUB;
    let step = SUB / 16;
    for sz in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let px = (x * SAMPLES + sx) as i32 * SUB / SAMPLES as i32 + SUB / (2 * SAMPLES as i32);
            let pz = (z * SAMPLES + sz) as i32 * SUB / SAMPLES as i32 + SUB / (2 * SAMPLES as i32);
            let mut found = None;
            'cells: for cy in (0..SIZE_Y as i32).rev() {
                let block = world.block([x as i32, cy, z as i32]);
                let bricks = world.bricks_in([x as i32, cy, z as i32], 0xF);
                if block.is_none() && !bricks {
                    continue;
                }
                for k in (0..16).rev() {
                    let p = [px, cy * SUB + k * step + step / 2, pz];
                    if let Some(b) = world.bricks.iter().find(|b| b.contains(p)) {
                        found = Some((
                            cy as f32 + (k + 1) as f32 / 16.0,
                            map.top_colour[b.id as usize],
                        ));
                        break 'cells;
                    }
                    if world.terrain_solid(p) {
                        let Some((b, _)) = block else { continue };
                        if b.id == 3 || b.id == 4 {
                            continue 'cells;
                        }
                        let colour = match b.id {
                            0 => HATCH,
                            1 => EXIT,
                            id => map.top_colour[id as usize],
                        };
                        found = Some((cy as f32 + (k + 1) as f32 / 16.0, colour));
                        break 'cells;
                    }
                }
            }
            let i = (z * SAMPLES + sz) * GRID + x * SAMPLES + sx;
            let (h, c) = found.unwrap_or((f32::NAN, map.ground[z * SIZE_X + x]));
            detail.height[i] = h;
            detail.colour[i] = c;
        }
    }
}

/// Enhanced mode: samples again the columns whose blocks or bricks changed
/// since they were last sampled (all of them the first time).
fn refresh_detail(map: &mut Minimap, world: &l3d_sim::World) {
    let mut detail = map.detail.take().unwrap_or_else(|| Detail {
        height: vec![f32::NAN; GRID * GRID],
        colour: vec![[0; 3]; GRID * GRID],
        digest: vec![u64::MAX; SIZE_X * SIZE_Z],
    });
    for z in 0..SIZE_Z {
        for x in 0..SIZE_X {
            let digest = column_digest(world, x as i32, z as i32);
            if detail.digest[z * SIZE_X + x] != digest {
                detail.digest[z * SIZE_X + x] = digest;
                sample_column(map, world, x, z, &mut detail);
            }
        }
    }
    map.detail = Some(detail);
}

/// Redraws the map with the lemmings and the camera: the original's, or in
/// enhanced mode the detailed one.
pub(crate) fn draw(w: &mut World) {
    let enhanced = w.settings.enhanced();
    let side = if enhanced { MAP * HI } else { MAP };
    let (Some(map), Some(sim)) = (&mut w.minimap, &w.game.sim) else {
        return;
    };
    if w.e
        .images
        .get(&map.image)
        .is_none_or(|i| i.width as usize != side)
    {
        let _ = w.e.images.insert(
            &map.image,
            rgba_image(side as u32, side as u32, vec![255; side * side * 4]),
        );
    }
    if enhanced {
        refresh_detail(map, &sim.world);
        let Some(image) = w.e.images.get_mut(&map.image) else {
            return;
        };
        draw_detailed(map, sim, &w.view, &mut image.data);
        return;
    }
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

/// Enhanced mode: draws the sampled terrain, shaded by height and by its
/// slope lit from the map's top left, with dark edges where the top steps,
/// then hatches, exits, objects, the camera's view cone and the lemmings.
fn draw_detailed(
    map: &Minimap,
    sim: &l3d_sim::Simulation,
    view: &crate::ViewCamera,
    px: &mut [u8],
) {
    let Some(detail) = &map.detail else { return };
    let side = MAP * HI;
    let frame = MapFrame::of(view);
    let s = SAMPLES as f32;
    let at = |x: f32, z: f32| -> Option<usize> {
        let (i, j) = ((x * s).floor(), (z * s).floor());
        ((0.0..GRID as f32).contains(&i) && (0.0..GRID as f32).contains(&j))
            .then(|| j as usize * GRID + i as usize)
    };
    let height = |x: f32, z: f32| at(x, z).map_or(f32::NAN, |i| detail.height[i]);
    // The light comes from the map's top left, whichever way it is turned:
    // up less right, the map's right being (−up.z, up.x) in the level.
    let (ux, uz) = frame.up;
    let lit = ((ux + uz) * FRAC_1_SQRT_2, (uz - ux) * FRAC_1_SQRT_2);
    for v in 0..side {
        for u in 0..side {
            let (x, z) = frame.to_world((u as f32 + 0.5) / HI as f32, (v as f32 + 0.5) / HI as f32);
            let c = match at(x, z) {
                None => map.background,
                Some(i) => {
                    let h = detail.height[i];
                    let base = detail.colour[i];
                    if h.is_nan() {
                        base
                    } else {
                        let d = 1.0 / s;
                        let hh = |dx: f32, dz: f32| {
                            let n = height(x + dx, z + dz);
                            if n.is_nan() { 0.0 } else { n }
                        };
                        let gx = hh(d, 0.0) - hh(-d, 0.0);
                        let gz = hh(0.0, d) - hh(0.0, -d);
                        // Facing the light brightens, facing away darkens.
                        let slope = (1.0 - 1.5 * (gx * lit.0 + gz * lit.1)).clamp(0.55, 1.35);
                        // Higher tops a little brighter.
                        let lift = 0.75 + 0.25 * (h / 8.0).min(1.0);
                        // An edge where a neighbour is much lower or higher.
                        let step = [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)]
                            .iter()
                            .map(|&(dx, dz)| (hh(dx, dz) - h).abs())
                            .fold(0.0f32, f32::max);
                        let edge = if step > 0.3 { 0.55 } else { 1.0 };
                        let k = slope * lift * edge;
                        base.map(|c| (c as f32 * k).clamp(0.0, 255.0) as u8)
                    }
                }
            };
            let i = (v * side + u) * 4;
            px[i..i + 3].copy_from_slice(&c);
        }
    }
    let put = |px: &mut [u8], u: isize, v: isize, c: [u8; 3]| {
        if (0..side as isize).contains(&u) && (0..side as isize).contains(&v) {
            let i = (v as usize * side + u as usize) * 4;
            px[i..i + 3].copy_from_slice(&c);
        }
    };
    let to_px = |x: f32, z: f32| {
        let (u, v) = frame.to_map(x, z);
        ((u * HI as f32) as isize, (v * HI as f32) as isize)
    };
    // Hatches and exits: framed squares.
    for &((x, z), c) in &map.marks {
        let (u, v) = to_px(x as f32 + 0.5, z as f32 + 0.5);
        for dv in -4..=4isize {
            for du in -4..=4isize {
                let rim = du.abs() == 4 || dv.abs() == 4;
                put(px, u + du, v + dv, if rim { [20, 20, 20] } else { c });
            }
        }
    }
    // Objects: small squares, magenta for traps that kill.
    let kills = sim.object_kind.is_some_and(|k| k.kills());
    for o in &sim.objects {
        let f = o.feet();
        let (u, v) = to_px(crate::units(f[0]), crate::units(f[2]));
        for dv in -2..=2isize {
            for du in -2..=2isize {
                put(px, u + du, v + dv, if kills { TRAP } else { OBJECT });
            }
        }
    }
    // The camera: a faint cone along its view, and a dot.
    let (cu, cv) = (CENTRE * HI as f32, CENTRE * HI as f32);
    for v in 0..side {
        for u in 0..side {
            let (du, dv) = (u as f32 + 0.5 - cu, v as f32 + 0.5 - cv);
            let r = du.hypot(dv);
            if r > 2.0 && r < CONE_LEN && (du.atan2(-dv)).abs() < CONE_HALF {
                let i = (v * side + u) * 4;
                for k in 0..3 {
                    px[i + k] = ((px[i + k] as u16 * 3 + CAMERA[k] as u16) / 4) as u8;
                }
            }
        }
    }
    for dv in -2..=2isize {
        for du in -2..=2isize {
            let i = ((cv as isize + dv) as usize * side + (cu as isize + du) as usize) * 4;
            px[i..i + 3].copy_from_slice(&CAMERA);
        }
    }
    // Lemmings: dots with a dark rim, coloured by what they do.
    for l in sim.lemmings.iter().filter(|l| !l.gone) {
        use l3d_sim::State::*;
        let c = match l.state {
            Blocking => BLOCKER,
            Turning { .. } => TURNER,
            Building { .. } | Shrugging => BUILDER,
            Digging | Bashing | Mining => DIGGING,
            Climbing | Floating => CLIMBING,
            _ => LEMMING,
        };
        let (x, z) = (crate::units(l.pos[0]), crate::units(l.pos[2]));
        let (u0, v0) = frame.to_map(x, z);
        let (u0, v0) = (u0 * HI as f32, v0 * HI as f32);
        for dv in -3..=3isize {
            for du in -3..=3isize {
                let r = ((du * du + dv * dv) as f32).sqrt();
                if r <= 3.2 {
                    let rim = r > 2.2;
                    put(
                        px,
                        u0 as isize + du,
                        v0 as isize + dv,
                        if rim { [20, 20, 20] } else { c },
                    );
                }
            }
        }
    }
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
