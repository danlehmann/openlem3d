//! Assembles a level's renderable content (blocks, sea, land, sky) from the
//! game data. Formats and conventions: `docs/spec/`.

use l3d_formats::gamedata::{GameData, Palette};
use l3d_formats::level::{Level, SIZE_X, SIZE_Z};

use crate::level_mesh::{self, LevelMesh};
use crate::scene_render::{RgbaImage, SceneData, SceneLayer, VERTEX_FLOATS};

/// Height of the ground plane (sea and land): the top of grid layer 0.
pub const GROUND_Y: f32 = 1.0;

/// Level header flag bits (`docs/spec/level.md`).
mod level_flags {
    pub const SEA_SOLID_COLOUR: u16 = 0x0001;
    pub const LAND_INVISIBLE: u16 = 0x0004;
    pub const LAND_128: u16 = 0x0100;
    pub const SURROUND_SKY: u16 = 0x0200;
    pub const LAND_HALF_SCALE: u16 = 0x0800;
}

/// How far the sea plane extends beyond the grid, in grid units.
const SEA_MARGIN: f32 = 4000.0;

/// Converts indexed pixels to RGBA; palette index 0 becomes transparent.
pub fn indexed_to_rgba(pixels: &[u8], width: u32, pal: &Palette) -> RgbaImage {
    let texels = pixels
        .iter()
        .flat_map(|&i| {
            let [r, g, b] = pal[i as usize];
            [r, g, b, if i == 0 { 0 } else { 255 }]
        })
        .collect();
    RgbaImage { width, height: pixels.len() as u32 / width, texels }
}

/// Appends a vertex in the scene renderer's layout.
fn push_vertex(out: &mut Vec<f32>, pos: [f32; 3], uv: [f32; 2], brightness: f32, cutout: bool) {
    out.extend(pos);
    out.extend(uv);
    out.extend([brightness, if cutout { 1.0 } else { 0.0 }]);
}

/// Interleaves the block mesh (opaque and cutout parts) into one layer.
fn block_layer(mesh: &LevelMesh, texture: RgbaImage) -> SceneLayer {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (part, cutout) in [(&mesh.opaque, false), (&mesh.cutout, true)] {
        let base = (vertices.len() / VERTEX_FLOATS) as u32;
        for i in 0..part.positions.len() {
            push_vertex(&mut vertices, part.positions[i], part.uvs[i], part.colors[i][0], cutout);
        }
        indices.extend(part.indices.iter().map(|i| i + base));
    }
    SceneLayer { vertices, indices, texture }
}

/// A large horizontal quad at `y` textured with `tile` world units per
/// texture repeat. Wound counter-clockwise seen from above.
fn plane_layer(y: f32, tile: f32, texture: RgbaImage) -> SceneLayer {
    let (lo_x, hi_x) = (-SEA_MARGIN, SIZE_X as f32 + SEA_MARGIN);
    let (lo_z, hi_z) = (-SEA_MARGIN, SIZE_Z as f32 + SEA_MARGIN);
    let mut vertices = Vec::new();
    for (x, z) in [(lo_x, lo_z), (lo_x, hi_z), (hi_x, hi_z), (hi_x, lo_z)] {
        push_vertex(&mut vertices, [x, y, z], [x / tile, z / tile], 1.0, false);
    }
    SceneLayer { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture }
}

/// The level's land polygons at ground height.
fn land_layer(level: &Level, texture: RgbaImage, tile: f32) -> SceneLayer {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for poly in &level.land_polygons {
        if poly.len() < 3 {
            continue;
        }
        let base = (vertices.len() / VERTEX_FLOATS) as u32;
        // The top 3 bits of the options byte darken the texture (unverified).
        let brightness = 1.0 - (poly[0].options >> 5) as f32 * 0.07;
        let pts: Vec<[f32; 3]> = poly.iter().map(|v| [v.x as f32, GROUND_Y + 0.002, v.z as f32]).collect();
        for p in &pts {
            push_vertex(&mut vertices, *p, [p[0] / tile, p[2] / tile], brightness, false);
        }
        // Fan triangulation, wound so the face points up (+Y).
        let up = {
            let (a, b, c) = (pts[0], pts[1], pts[2]);
            let (u, v) = ([b[0] - a[0], b[2] - a[2]], [c[0] - a[0], c[2] - a[2]]);
            // y component of (b − a) × (c − a)
            u[1] * v[0] - u[0] * v[1] > 0.0
        };
        for i in 1..pts.len() as u32 - 1 {
            if up {
                indices.extend([base, base + i, base + i + 1]);
            } else {
                indices.extend([base, base + i + 1, base + i]);
            }
        }
    }
    SceneLayer { vertices, indices, texture }
}

/// Builds everything drawn for level `n`.
pub fn build(data: &mut GameData, n: u32) -> Result<(Level, SceneData, LevelMesh), l3d_formats::Error> {
    let level = data.level(n)?;
    let blocks = data.blocks(n)?;
    let pal = data.palette("GFX/LM3D.PAL")?;
    let mesh = level_mesh::build(&level, &blocks);
    let mut scene = SceneData::default();

    let tex = data.gfx("TEXTURE", level.texture_set)?;
    let flags = level.flags;
    let surround = flags & level_flags::SURROUND_SKY != 0;

    if level.sea_gfx != 0xFF && !surround && flags & level_flags::SEA_SOLID_COLOUR == 0
        && let Ok(sea) = data.gfx("SEA", level.sea_gfx) {
            // First 64×64 frame; one texture repeat per grid unit (unverified).
            let frame = &sea[..64 * 64.min(sea.len() / 64)];
            scene.layers.push(plane_layer(GROUND_Y - 0.01, 1.0, indexed_to_rgba(frame, 64, &pal)));
        }
    if level.land_gfx != 0xFF && flags & level_flags::LAND_INVISIBLE == 0 && !level.land_polygons.is_empty()
        && let Ok(land) = data.gfx("LAND", level.land_gfx) {
            // LAND files are 128 wide; use the first texture. Texel density
            // relative to the grid is unverified: 128 texels span 2 units
            // when the "128×128" flag is set, else 1 unit.
            let first = &land[..(128 * 128).min(land.len())];
            let mut tile = if flags & level_flags::LAND_128 != 0 { 2.0 } else { 1.0 };
            if flags & level_flags::LAND_HALF_SCALE != 0 {
                tile /= 2.0;
            }
            scene.layers.push(land_layer(&level, indexed_to_rgba(first, 128, &pal), tile));
        }
    scene.layers.push(block_layer(&mesh, indexed_to_rgba(&tex, 64, &pal)));

    if level.sky_gfx != 0xFF
        && let Ok(sky) = data.gfx("SKY", level.sky_gfx) {
            // 1024×64 panoramas only; 200×320 all-round skies are not handled yet.
            if sky.len() == 1024 * 64 {
                scene.sky = Some(indexed_to_rgba(&sky, 1024, &pal));
            }
        }
    Ok((level, scene, mesh))
}
