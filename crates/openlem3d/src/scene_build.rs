//! Assembles a level's renderable content (blocks, sea, land, sky, sprite
//! objects) from the game data. Formats and conventions: `docs/spec/`.

use l3d_formats::blk::BlockSet;
use l3d_formats::gamedata::{GameData, Palette};
use l3d_formats::level::{BlockCell, Level, SIZE_X, SIZE_Z};

use crate::level_mesh::{self, LevelMesh};
use crate::scene_render::{RgbaImage, SceneData, SceneLayer, VERTEX_FLOATS};

/// Height of the ground plane (sea and land): the top of grid layer 0.
pub const GROUND_Y: f32 = 1.0;

/// Texels per grid unit for sprites (same density as block faces).
const SPRITE_TEXELS_PER_UNIT: f32 = 64.0;

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

/// One vertex in the scene renderer's layout.
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    uv: [f32; 2],
    brightness: f32,
    cutout: bool,
    /// Billboard offset along camera-right and up.
    offset: [f32; 2],
    /// Animation frame count and uv stride between frames.
    anim: [f32; 2],
}

impl Vertex {
    fn fixed(pos: [f32; 3], uv: [f32; 2], brightness: f32, cutout: bool) -> Self {
        Vertex { pos, uv, brightness, cutout, offset: [0.0; 2], anim: [1.0, 0.0] }
    }

    fn push(&self, out: &mut Vec<f32>) {
        out.extend(self.pos);
        out.extend(self.uv);
        out.extend([self.brightness, if self.cutout { 1.0 } else { 0.0 }]);
        out.extend(self.offset);
        out.extend(self.anim);
    }
}

/// Accumulates vertices and indices for one layer.
#[derive(Default)]
pub struct LayerBuilder {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

impl LayerBuilder {
    fn base(&self) -> u32 {
        (self.vertices.len() / VERTEX_FLOATS) as u32
    }

    /// Adds a camera-facing sprite whose bottom centre sits at `anchor`.
    /// `rect` is the sprite's texel rectangle `[x, y, w, h]` in a texture of
    /// `tex_size`; `frames` > 1 animates by stepping down `frame_stride` texels.
    fn sprite(&mut self, anchor: [f32; 3], rect: [f32; 4], tex_size: [f32; 2], frames: u32, frame_stride: f32) {
        self.sprite_scaled(anchor, rect, tex_size, frames, frame_stride, SPRITE_TEXELS_PER_UNIT);
    }

    /// Like [`Self::sprite`] with an explicit texel density.
    pub fn sprite_scaled(
        &mut self,
        anchor: [f32; 3],
        rect: [f32; 4],
        tex_size: [f32; 2],
        frames: u32,
        frame_stride: f32,
        texels_per_unit: f32,
    ) {
        let [x, y, w, h] = rect;
        let (hw, hh) = (w / texels_per_unit / 2.0, h / texels_per_unit);
        let (u0, u1) = (x / tex_size[0], (x + w) / tex_size[0]);
        let (v0, v1) = (y / tex_size[1], (y + h) / tex_size[1]);
        let anim = [frames as f32, frame_stride / tex_size[1]];
        let base = self.base();
        // Counter-clockwise as seen from the camera: right is +offset.x.
        for (off, uv) in [([-hw, 0.0], [u0, v1]), ([hw, 0.0], [u1, v1]), ([hw, hh], [u1, v0]), ([-hw, hh], [u0, v0])] {
            Vertex { pos: anchor, uv, brightness: 1.0, cutout: true, offset: off, anim }.push(&mut self.vertices);
        }
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn finish(self, texture: RgbaImage) -> SceneLayer {
        SceneLayer { vertices: self.vertices, indices: self.indices, texture }
    }
}

/// Interleaves the block mesh (opaque and cutout parts) into one layer.
fn block_layer(mesh: &LevelMesh, texture: RgbaImage) -> SceneLayer {
    let mut b = LayerBuilder::default();
    for (part, cutout) in [(&mesh.opaque, false), (&mesh.cutout, true)] {
        let base = b.base();
        for i in 0..part.positions.len() {
            Vertex::fixed(part.positions[i], part.uvs[i], part.colors[i][0], cutout).push(&mut b.vertices);
        }
        b.indices.extend(part.indices.iter().map(|i| i + base));
    }
    b.finish(texture)
}

/// A large horizontal quad at `y` textured with `tile` world units per
/// texture repeat. Wound counter-clockwise seen from above.
fn plane_layer(y: f32, tile: f32, texture: RgbaImage) -> SceneLayer {
    let (lo_x, hi_x) = (-SEA_MARGIN, SIZE_X as f32 + SEA_MARGIN);
    let (lo_z, hi_z) = (-SEA_MARGIN, SIZE_Z as f32 + SEA_MARGIN);
    let mut b = LayerBuilder::default();
    for (x, z) in [(lo_x, lo_z), (lo_x, hi_z), (hi_x, hi_z), (hi_x, lo_z)] {
        Vertex::fixed([x, y, z], [x / tile, z / tile], 1.0, false).push(&mut b.vertices);
    }
    b.indices = vec![0, 1, 2, 0, 2, 3];
    b.finish(texture)
}

/// The level's land polygons at ground height.
fn land_layer(level: &Level, texture: RgbaImage, tile: f32) -> SceneLayer {
    let mut b = LayerBuilder::default();
    for poly in &level.land_polygons {
        if poly.len() < 3 {
            continue;
        }
        let base = b.base();
        // The top 3 bits of the options byte darken the texture (unverified).
        let brightness = 1.0 - (poly[0].options >> 5) as f32 * 0.07;
        let pts: Vec<[f32; 3]> = poly.iter().map(|v| [v.x as f32, GROUND_Y + 0.002, v.z as f32]).collect();
        for p in &pts {
            Vertex::fixed(*p, [p[0] / tile, p[2] / tile], brightness, false).push(&mut b.vertices);
        }
        // Fan triangulation, wound so the face points up (+Y).
        let up = {
            let (a, c1, c2) = (pts[0], pts[1], pts[2]);
            let (u, v) = ([c1[0] - a[0], c1[2] - a[2]], [c2[0] - a[0], c2[2] - a[2]]);
            // y component of (c1 − a) × (c2 − a)
            u[1] * v[0] - u[0] * v[1] > 0.0
        };
        for i in 1..pts.len() as u32 - 1 {
            if up {
                b.indices.extend([base, base + i, base + i + 1]);
            } else {
                b.indices.extend([base, base + i + 1, base + i]);
            }
        }
    }
    b.finish(texture)
}

/// Texel rectangle `[x, y, w, h]` of static object `n` (1-based) in a
/// 64×704 `OBJ` sheet (`docs/spec/graphics.md`).
fn obj_rect(n: u8) -> [f32; 4] {
    let n = if (1..=20).contains(&n) { n } else { 1 };
    let i = (n - 1) % 4;
    let (half_x, half_y) = ((i % 2) as f32 * 32.0, (i / 2) as f32);
    match n {
        1..=4 => [half_x, half_y * 64.0, 32.0, 64.0],
        5..=8 => [0.0, 128.0 + i as f32 * 64.0, 64.0, 64.0],
        9..=12 => [half_x, 384.0 + half_y * 64.0, 32.0, 64.0],
        13..=16 => [0.0, 512.0 + i as f32 * 32.0, 64.0, 32.0],
        _ => [half_x, 640.0 + half_y * 32.0, 32.0, 32.0],
    }
}

/// Height at which an object in a cell stands: the bottom of the cell, or the
/// top of a partial block whose bottom slice is present ([L3DEdit]
/// "Objects vs Blocks", unverified).
fn object_base_y(y: usize, block: BlockCell) -> f32 {
    let partial = block.segments != 0 && block.segments != 0xF;
    if partial && block.segments & 1 != 0 {
        y as f32 + (8 - block.segments.leading_zeros()) as f32 / 4.0
    } else {
        y as f32
    }
}

/// Sprite layers for static (`OBJ`) and animated (`ANIMOBJ`) objects.
fn object_layers(data: &mut GameData, level: &Level, pal: &Palette) -> Vec<SceneLayer> {
    let mut layers = Vec::new();
    let mut statics = LayerBuilder::default();
    let mut animated = LayerBuilder::default();
    let obj = (level.object_set != 0xFF).then(|| data.gfx("OBJ", level.object_set).ok()).flatten();
    let anim = (level.anim_object != 0xFF).then(|| data.gfx("ANIMOBJ", level.anim_object).ok()).flatten();
    let obj_size = [64.0, obj.as_ref().map_or(704, |o| o.len() / 64) as f32];
    let anim_size = [64.0, anim.as_ref().map_or(256, |a| a.len() / 64) as f32];
    for (x, y, z, block, o) in level.cells() {
        let (cx, cz) = (x as f32 + 0.5, z as f32 + 0.5);
        let base_y = object_base_y(y, block);
        match o.kind {
            0x01..=0x1F if obj.is_some() => {
                let n = if o.kind > 0x14 { 1 } else { o.kind };
                statics.sprite([cx, base_y, cz], obj_rect(n), obj_size, 1, 0.0);
            }
            0x50..=0x5B => {
                // 0x51 animates the four 64×64 OBJ frames; the rest use ANIMOBJ.
                let (dx, dz, half) = match o.kind {
                    0x52 => (0.5, 0.5, None),
                    0x53 => (0.5, -0.5, None),
                    0x54..=0x5B => {
                        let corner = [(-0.5, -0.5), (-0.5, 0.5), (0.5, 0.5), (0.5, -0.5)][(o.kind as usize - 0x54) % 4];
                        (corner.0, corner.1, Some(if o.kind < 0x58 { 0.0 } else { 32.0 }))
                    }
                    _ => (0.0, 0.0, None),
                };
                let anchor = [cx + dx, base_y, cz + dz];
                if o.kind == 0x51 {
                    if obj.is_some() {
                        statics.sprite(anchor, [0.0, 128.0, 64.0, 64.0], obj_size, 4, 64.0);
                    }
                } else if anim.is_some() {
                    let rect = match half {
                        Some(hx) => [hx, 0.0, 32.0, 64.0],
                        None => [0.0, 0.0, 64.0, 64.0],
                    };
                    animated.sprite(anchor, rect, anim_size, (anim_size[1] / 64.0) as u32, 64.0);
                }
            }
            _ => {}
        }
    }
    if let Some(obj) = obj
        && !statics.indices.is_empty()
    {
        layers.push(statics.finish(indexed_to_rgba(&obj, 64, pal)));
    }
    if let Some(anim) = anim
        && !animated.indices.is_empty()
    {
        layers.push(animated.finish(indexed_to_rgba(&anim, 64, pal)));
    }
    layers
}

/// Cells per row in the lemming atlas.
pub const ATLAS_COLUMNS: u32 = 32;
/// Size of one lemming cell in `LEMM.MHC`.
pub const LEMMING_CELL: u32 = 64;

/// All `LEMM.MHC` cells packed into one texture, [`ATLAS_COLUMNS`] per row;
/// cell `i` is at column `i % ATLAS_COLUMNS`, row `i / ATLAS_COLUMNS`.
fn lemming_atlas(data: &mut GameData, pal: &Palette) -> Result<RgbaImage, l3d_formats::Error> {
    let raw = data.read("LEMM/LEMM.MHC")?;
    let mhc = l3d_formats::mhc::MhcFile::parse(&raw, LEMMING_CELL as usize)?;
    let n = mhc.entries.len() as u32;
    let (w, h) = (ATLAS_COLUMNS * LEMMING_CELL, n.div_ceil(ATLAS_COLUMNS) * LEMMING_CELL);
    let mut pixels = vec![0u8; (w * h) as usize];
    for i in 0..n {
        let cell = mhc.cell(i as usize)?;
        let (ox, oy) = ((i % ATLAS_COLUMNS) * LEMMING_CELL, (i / ATLAS_COLUMNS) * LEMMING_CELL);
        for y in 0..LEMMING_CELL {
            let dst = ((oy + y) * w + ox) as usize;
            let src = (y * LEMMING_CELL) as usize;
            pixels[dst..dst + LEMMING_CELL as usize].copy_from_slice(&cell.pixels[src..src + LEMMING_CELL as usize]);
        }
    }
    Ok(indexed_to_rgba(&pixels, w, pal))
}

/// A loaded level with its renderable content.
pub struct BuiltLevel {
    pub level: Level,
    pub blocks: BlockSet,
    pub scene: SceneData,
    pub mesh: LevelMesh,
    /// Index in `scene.layers` of the block geometry.
    pub block_layer: usize,
}

/// Replaces the block geometry of `scene` with a mesh of the current grid.
pub fn rebuild_blocks(scene: &SceneData, layer: usize, level: &Level, blocks: &BlockSet) -> SceneData {
    let mut out = scene.clone();
    let texture = out.layers[layer].texture.clone();
    out.layers[layer] = block_layer(&level_mesh::build(level, blocks), texture);
    out
}

/// Builds everything drawn for level `n`.
pub fn build(data: &mut GameData, n: u32) -> Result<BuiltLevel, l3d_formats::Error> {
    let level = data.level(n)?;
    let blocks = data.blocks(n)?;
    let pal = data.palette("GFX/LM3D.PAL")?;
    let mesh = level_mesh::build(&level, &blocks);
    let mut scene = SceneData::default();

    let tex = data.gfx("TEXTURE", level.texture_set)?;
    let flags = level.flags;
    let surround = flags & level_flags::SURROUND_SKY != 0;

    if level.sea_gfx != 0xFF
        && !surround
        && flags & level_flags::SEA_SOLID_COLOUR == 0
        && let Ok(sea) = data.gfx("SEA", level.sea_gfx)
    {
        // First 64×64 frame; one texture repeat per grid unit (unverified).
        let frame = &sea[..64 * 64.min(sea.len() / 64)];
        scene.layers.push(plane_layer(GROUND_Y - 0.01, 1.0, indexed_to_rgba(frame, 64, &pal)));
    }
    if level.land_gfx != 0xFF
        && flags & level_flags::LAND_INVISIBLE == 0
        && !level.land_polygons.is_empty()
        && let Ok(land) = data.gfx("LAND", level.land_gfx)
    {
        // LAND files are 128 wide; use the first texture. Texel density
        // relative to the grid is unverified: 128 texels span 2 units when
        // the "128×128" flag is set, else 1 unit.
        let first = &land[..(128 * 128).min(land.len())];
        let mut tile = if flags & level_flags::LAND_128 != 0 { 2.0 } else { 1.0 };
        if flags & level_flags::LAND_HALF_SCALE != 0 {
            tile /= 2.0;
        }
        scene.layers.push(land_layer(&level, indexed_to_rgba(first, 128, &pal), tile));
    }
    let block_layer_index = scene.layers.len();
    scene.layers.push(block_layer(&mesh, indexed_to_rgba(&tex, 64, &pal)));
    scene.layers.extend(object_layers(data, &level, &pal));

    scene.sprite_atlas = lemming_atlas(data, &pal).ok();

    if level.sky_gfx != 0xFF
        && let Ok(sky) = data.gfx("SKY", level.sky_gfx)
        && sky.len() == 1024 * 64
    {
        // 1024×64 panoramas only; 200×320 all-round skies are not handled yet.
        scene.sky = Some(indexed_to_rgba(&sky, 1024, &pal));
    }
    Ok(BuiltLevel { level, blocks, scene, mesh, block_layer: block_layer_index })
}
