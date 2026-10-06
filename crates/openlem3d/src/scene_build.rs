//! Assembles a level's renderable content (blocks, sea, land, sky, sprite
//! objects) from the game data. Formats and conventions: `docs/spec/`.

use l3d_formats::blk::BlockSet;
use l3d_formats::gamedata::{GameData, Palette};
use l3d_formats::level::{BlockCell, LandVertex, Level, SIZE_X, SIZE_Z};

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
    pub const SEA_STILL: u16 = 0x0010;
    pub const SEA_128: u16 = 0x0020;
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
        // A negative width mirrors the image horizontally.
        let (hw, hh) = (w.abs() / texels_per_unit / 2.0, h / texels_per_unit);
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

    /// Adds a fixed quad with corners bottom-left, bottom-right, top-right,
    /// top-left as seen from its front, showing texel rectangle `rect`
    /// (`[x, y, w, h]`) of a texture of `tex_size`.
    fn quad(&mut self, corners: [[f32; 3]; 4], rect: [f32; 4], tex_size: [f32; 2], brightness: f32) {
        let [x, y, w, h] = rect;
        let (u0, u1) = (x / tex_size[0], (x + w) / tex_size[0]);
        let (v0, v1) = (y / tex_size[1], (y + h) / tex_size[1]);
        let base = self.base();
        for (p, uv) in corners.iter().zip([[u0, v1], [u1, v1], [u1, v0], [u0, v0]]) {
            Vertex::fixed(*p, uv, brightness, true).push(&mut self.vertices);
        }
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A vertical quad whose bottom edge is centred on `centre`, facing
    /// horizontal direction `normal`, `width` × `height` units.
    #[allow(clippy::too_many_arguments)]
    fn vertical_quad(&mut self, centre: [f32; 3], normal: [f32; 2], width: f32, height: f32, rect: [f32; 4], tex: [f32; 2], brightness: f32) {
        // Right as seen from the front: +Y × normal.
        let right = [normal[1], -normal[0]];
        let (hw, [cx, cy, cz]) = (width / 2.0, centre);
        let bl = [cx - right[0] * hw, cy, cz - right[1] * hw];
        let br = [cx + right[0] * hw, cy, cz + right[1] * hw];
        let up = |p: [f32; 3]| [p[0], p[1] + height, p[2]];
        self.quad([bl, br, up(br), up(bl)], rect, tex, brightness);
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
            let mut v = Vertex::fixed(part.positions[i], part.uvs[i], part.colors[i][0], cutout);
            v.anim = part.anims[i];
            v.push(&mut b.vertices);
        }
        b.indices.extend(part.indices.iter().map(|i| i + base));
    }
    b.finish(texture)
}

/// A large horizontal quad at `y` textured with `tile` world units per
/// texture repeat. Wound counter-clockwise seen from above.
///
/// The plane is split into tiles, small near the level and large far away:
/// a single huge quad loses so much depth precision when rasterised that the
/// sea showed through the land at some window sizes (seen at exactly
/// 1280×720 on Adreno/DX12).
/// `frames` stacked frames animate the texture and `drift` scrolls it (uv
/// per second).
fn plane_layer(y: f32, tile: f32, texture: RgbaImage, frames: u32, drift: [f32; 2]) -> SceneLayer {
    /// Size of the coarse tiles.
    const COARSE: f32 = 512.0;
    /// Size of the fine tiles used near the level.
    const FINE: f32 = 16.0;
    /// The region around the grid that gets fine tiles.
    const NEAR: f32 = 64.0;
    let near = (-NEAR, SIZE_X.max(SIZE_Z) as f32 + NEAR);
    let mut b = LayerBuilder::default();
    let quad = |b: &mut LayerBuilder, x0: f32, z0: f32, size: f32| {
        let base = b.base();
        for (x, z) in [(x0, z0), (x0, z0 + size), (x0 + size, z0 + size), (x0 + size, z0)] {
            let mut v = Vertex::fixed([x, y, z], [x / tile, z / tile], 1.0, false);
            // A tiled surface (see `scene.wgsl`): frames, and the drift.
            v.anim = [frames as f32, -1.0];
            v.offset = drift;
            v.push(&mut b.vertices);
        }
        b.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    let start = -(SEA_MARGIN / COARSE).floor() * COARSE;
    let steps = ((SEA_MARGIN * 2.0 + SIZE_X.max(SIZE_Z) as f32) / COARSE).ceil() as i32 + 1;
    for i in 0..steps {
        for j in 0..steps {
            let (x0, z0) = (start + i as f32 * COARSE, start + j as f32 * COARSE);
            let overlaps_near = x0 < near.1 && x0 + COARSE > near.0 && z0 < near.1 && z0 + COARSE > near.0;
            if overlaps_near {
                let n = (COARSE / FINE) as i32;
                for fi in 0..n {
                    for fj in 0..n {
                        quad(&mut b, x0 + fi as f32 * FINE, z0 + fj as f32 * FINE, FINE);
                    }
                }
            } else {
                quad(&mut b, x0, z0, COARSE);
            }
        }
    }
    b.finish(texture)
}

/// Which texture of a multi-texture `LAND` file a polygon shows: the low
/// three bits of its options byte. Unverified: inferred from LEMLAB and The
/// Catacombs, whose polygons use 1 with two-texture files, while every
/// single-texture level uses 0.
fn land_texture(poly: &[LandVertex]) -> usize {
    poly.first().map_or(0, |v| (v.options & 7) as usize)
}

/// Land polygons at ground height.
fn land_layer(polygons: &[&Vec<LandVertex>], texture: RgbaImage, tile: f32) -> SceneLayer {
    let mut b = LayerBuilder::default();
    for poly in polygons {
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

/// Interactive-object type values (header `0x114`, [L3DEdit]).
const TRAP_TRAMPOLINE: u8 = 3;

/// Height of the top of the block in a cell, if the cell holds one.
fn block_top(y: usize, block: BlockCell) -> Option<f32> {
    (block.segments != 0).then(|| y as f32 + (8 - block.segments.leading_zeros()) as f32 / 4.0)
}

/// Trampoline pads (`0x60`–`0x67` on a trampoline level) from the level's
/// `TRAPS` sheet of 64×64 frames, flat on top of their block ([L3DEdit],
/// unverified). Other interactive objects are sprites drawn each frame with
/// the lemmings (`lemming_render`), so they can animate.
fn trap_layer(data: &mut GameData, level: &Level, pal: &Palette) -> Option<SceneLayer> {
    if level.trap_type != TRAP_TRAMPOLINE {
        return None;
    }
    let traps = data.gfx("TRAPS", level.trap_type).ok()?;
    let tex = [64.0, (traps.len() / 64) as f32];
    let mut b = LayerBuilder::default();
    for (x, y, z, block, o) in level.cells() {
        if !(0x60..=0x67).contains(&o.kind) {
            continue;
        }
        // Pads need a block to lie on ([L3DEdit]).
        let Some(top) = block_top(y, block) else { continue };
        // Red pads use frames 0–3, blue pads frames 4–7.
        let frame = if o.kind >= 0x64 { 4.0 } else { 0.0 };
        let y = top + DECAL_OFFSET;
        let (x0, z0, x1, z1) = (x as f32, z as f32, x as f32 + 1.0, z as f32 + 1.0);
        b.quad([[x0, y, z1], [x1, y, z1], [x1, y, z0], [x0, y, z0]], [0.0, frame * 64.0, 64.0, 64.0], tex, 1.0);
    }
    (!b.indices.is_empty()).then(|| b.finish(indexed_to_rgba(&traps, 64, pal)))
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

/// Gap between a decal and the cell face it is painted on.
const DECAL_OFFSET: f32 = 0.004;

/// Horizontal outward normals of a cell's sides in the order signs and walls
/// number them: +X, −Z, −X, +Z.
const SIDE_NORMALS: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, -1.0], [-1.0, 0.0], [0.0, 1.0]];

/// Texel rectangle of sign graphic `g` (0–15) in a 64×768 `SIGNS` sheet:
/// graphics 0–7 are 64×32, 8–15 are 64×64.
fn sign_rect(g: u8) -> [f32; 4] {
    if g < 8 { [0.0, g as f32 * 32.0, 64.0, 32.0] } else { [0.0, 256.0 + (g - 8) as f32 * 64.0, 64.0, 64.0] }
}

/// Sign (`0x20`–`0x3F`) and wall-decal (`0x70`–`0xEF`) layers
/// ([L3DEdit] semantics, unverified).
fn decal_layers(data: &mut GameData, level: &Level, pal: &Palette) -> Vec<SceneLayer> {
    let signs = (level.sign_set != 0xFF).then(|| data.gfx("SIGNS", level.sign_set).ok()).flatten();
    let walls = (level.walls_set != 0xFF).then(|| data.gfx("WALLS", level.walls_set).ok()).flatten();
    let sign_tex = [64.0, signs.as_ref().map_or(768, |s| s.len() / 64) as f32];
    let wall_tex = [64.0, walls.as_ref().map_or(1024, |w| w.len() / 64) as f32];
    let (mut sign_b, mut wall_b) = (LayerBuilder::default(), LayerBuilder::default());
    for (x, y, z, block, o) in level.cells() {
        let base_y = object_base_y(y, block);
        let (cx, cz) = (x as f32 + 0.5, z as f32 + 0.5);
        match o.kind {
            0x20..=0x3F if signs.is_some() => {
                // Low 2 bits: the edge; bits 2–3: the graphic pair. The front
                // graphic faces out of the cell, the back one into it.
                let n = SIDE_NORMALS[(o.kind & 3) as usize];
                let pair = (o.kind >> 2) & 3;
                let (front, back) = if o.kind < 0x30 {
                    (8 + pair, 12 + pair)
                } else {
                    [(0, 2), (1, 3), (4, 6), (5, 7)][pair as usize]
                };
                let at = |d: f32| [cx + n[0] * d, base_y, cz + n[1] * d];
                let (fr, br) = (sign_rect(front), sign_rect(back));
                sign_b.vertical_quad(at(0.5 + DECAL_OFFSET), n, 1.0, fr[3] / 64.0, fr, sign_tex, 1.0);
                sign_b.vertical_quad(at(0.5 - DECAL_OFFSET), [-n[0], -n[1]], 1.0, br[3] / 64.0, br, sign_tex, 1.0);
            }
            0x70..=0xEF if walls.is_some() => {
                let rect = [0.0, (o.kind & 0xF) as f32 * 64.0, 64.0, 64.0];
                let (x0, z0, x1, z1) = (x as f32, z as f32, x as f32 + 1.0, z as f32 + 1.0);
                match o.kind >> 4 {
                    side @ 0x7..=0xA => {
                        let n = SIDE_NORMALS[(side - 0x7) as usize];
                        let c = [cx + n[0] * (0.5 + DECAL_OFFSET), base_y, cz + n[1] * (0.5 + DECAL_OFFSET)];
                        wall_b.vertical_quad(c, n, 1.0, 1.0, rect, wall_tex, 1.0);
                    }
                    0xB => {
                        // Underside; the image's bottom edge faces −X.
                        let y = base_y - DECAL_OFFSET;
                        wall_b.quad([[x0, y, z1], [x0, y, z0], [x1, y, z0], [x1, y, z1]], rect, wall_tex, 1.0);
                    }
                    0xC => {
                        // Top; the image's bottom edge faces −X.
                        let y = base_y + 1.0 + DECAL_OFFSET;
                        wall_b.quad([[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]], rect, wall_tex, 1.0);
                    }
                    half @ (0xD | 0xE) => {
                        // Half-height, darkened panel through the middle of the
                        // cell: along Z (0xD) or along X (0xE), seen from both sides.
                        let n = if half == 0xD { [1.0, 0.0] } else { [0.0, 1.0] };
                        let c = [cx, base_y, cz];
                        wall_b.vertical_quad(c, n, 1.0, 0.5, rect, wall_tex, 0.6);
                        wall_b.vertical_quad(c, [-n[0], -n[1]], 1.0, 0.5, rect, wall_tex, 0.6);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let mut layers = Vec::new();
    if let Some(s) = signs
        && !sign_b.indices.is_empty()
    {
        layers.push(sign_b.finish(indexed_to_rgba(&s, 64, pal)));
    }
    if let Some(w) = walls
        && !wall_b.indices.is_empty()
    {
        layers.push(wall_b.finish(indexed_to_rgba(&w, 64, pal)));
    }
    layers
}

/// The first atlas cell holding the level's `TRAPS` frames (64×64 each),
/// after the lemming cells; [`TRAP_ATLAS_CELLS`] are reserved.
pub const TRAP_ATLAS_FIRST: u32 = (l3d_formats::mhc::MANIFEST_ENTRIES as u32).div_ceil(ATLAS_COLUMNS) * ATLAS_COLUMNS;
pub const TRAP_ATLAS_CELLS: u32 = 8;
/// The first atlas cell holding `BOMBNUMB.GFX` (32×32 each, in the top-left
/// of their cells): the countdown digits 1–5, then `?`, an arrow, a blank.
pub const BOMBNUMB_ATLAS_FIRST: u32 = TRAP_ATLAS_FIRST + TRAP_ATLAS_CELLS;
/// Rows in the sprite atlas.
pub const ATLAS_ROWS: u32 = (BOMBNUMB_ATLAS_FIRST + 8).div_ceil(ATLAS_COLUMNS);

/// Cells per row in the lemming atlas.
pub const ATLAS_COLUMNS: u32 = 32;
/// Size of one lemming cell in `LEMM.MHC`.
pub const LEMMING_CELL: u32 = 128;
/// Size of a `TRAPS` frame.
pub const TRAP_FRAME: u32 = 64;

/// All `LEMM.MHC` cells packed into one texture, [`ATLAS_COLUMNS`] per row
/// (cell `i` at column `i % ATLAS_COLUMNS`, row `i / ATLAS_COLUMNS`), then
/// the level's `TRAPS` frames from [`TRAP_ATLAS_FIRST`].
fn lemming_atlas(data: &mut GameData, pal: &Palette, traps: Option<&[u8]>) -> Result<RgbaImage, l3d_formats::Error> {
    // The 128-pixel sprites; the 64-pixel set (same cells) doubled if missing.
    let (raw, scale) = match data.read("LEMM/LEMM128.MHC") {
        Ok(raw) => (raw, 1),
        Err(_) => (data.read("LEMM/LEMM.MHC")?, 2),
    };
    let mhc = l3d_formats::mhc::MhcFile::parse(&raw, (LEMMING_CELL / scale) as usize)?;
    let n = mhc.entries.len() as u32;
    let (w, h) = (ATLAS_COLUMNS * LEMMING_CELL, ATLAS_ROWS * LEMMING_CELL);
    let mut pixels = vec![0u8; (w * h) as usize];
    // Copies a `size`-wide square image into the top-left of cell `i`.
    let mut put_sized = |i: u32, cell: &[u8], size: u32| {
        let (ox, oy) = ((i % ATLAS_COLUMNS) * LEMMING_CELL, (i / ATLAS_COLUMNS) * LEMMING_CELL);
        for y in 0..size {
            let dst = ((oy + y) * w + ox) as usize;
            let src = (y * size) as usize;
            pixels[dst..dst + size as usize].copy_from_slice(&cell[src..src + size as usize]);
        }
    };
    let mut put = |i: u32, cell: &[u8]| put_sized(i, cell, LEMMING_CELL);
    for i in 0..n {
        let cell = mhc.cell(i as usize)?;
        if scale == 1 {
            put(i, &cell.pixels);
        } else {
            let s = cell.size;
            let doubled: Vec<u8> = (0..s * 2).flat_map(|y| (0..s * 2).map(move |x| (x, y))).map(|(x, y)| cell.pixels[(y / 2) * s + x / 2]).collect();
            put(i, &doubled);
        }
    }
    let frame = (TRAP_FRAME * TRAP_FRAME) as usize;
    for (k, f) in traps.unwrap_or_default().chunks_exact(frame).take(TRAP_ATLAS_CELLS as usize).enumerate() {
        put_sized(TRAP_ATLAS_FIRST + k as u32, f, TRAP_FRAME);
    }
    let sheet = &l3d_formats::sheets::BOMBNUMB;
    if let Ok(digits) = data.read(sheet.path).and_then(|raw| sheet.cut(&raw)) {
        for (k, d) in digits.iter().take(8).enumerate() {
            put_sized(BOMBNUMB_ATLAS_FIRST + k as u32, &d.pixels, d.width as u32);
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
pub fn rebuild_blocks(scene: &SceneData, layer: usize, level: &Level, blocks: &BlockSet, bricks: &[l3d_sim::Brick]) -> SceneData {
    let mut out = scene.clone();
    let texture = out.layers[layer].texture.clone();
    out.layers[layer] = block_layer(&level_mesh::build(level, blocks), texture);
    out.layers[layer + 1] = brick_layer(bricks);
    out
}

/// Builders' bricks as brown boxes. The original's bricks are thin and brown
/// (`docs/spec/behaviour.md`); the texture is our own.
fn brick_layer(bricks: &[l3d_sim::Brick]) -> SceneLayer {
    const W: u32 = 16;
    const H: u32 = 4;
    let texels = (0..W * H)
        .flat_map(|i| {
            let (x, y) = (i % W, i / W);
            // Mortar lines between two courses of staggered bricks.
            let mortar = y % 2 == 1 || (x + if y < 2 { 0 } else { 4 }) % 8 == 7;
            if mortar { [92, 52, 24, 255] } else { [156, 92, 44, 255] }
        })
        .collect();
    let mut b = LayerBuilder::default();
    let sub = l3d_sim::SUB as f32;
    for brick in bricks {
        let (lo, hi) = (brick.min.map(|v| v as f32 / sub), brick.max.map(|v| v as f32 / sub));
        let p = |x: usize, y: usize, z: usize| [[lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z]];
        // Each face counter-clockwise from outside, with its brightness.
        let faces = [
            ([p(0, 1, 0), p(0, 1, 1), p(1, 1, 1), p(1, 1, 0)], 1.0),
            ([p(0, 0, 0), p(1, 0, 0), p(1, 0, 1), p(0, 0, 1)], 0.5),
            ([p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)], 0.8),
            ([p(1, 0, 0), p(0, 0, 0), p(0, 1, 0), p(1, 1, 0)], 0.8),
            ([p(1, 0, 1), p(1, 0, 0), p(1, 1, 0), p(1, 1, 1)], 0.7),
            ([p(0, 0, 0), p(0, 0, 1), p(0, 1, 1), p(0, 1, 0)], 0.7),
        ];
        for (corners, brightness) in faces {
            let base = b.base();
            for (c, uv) in corners.iter().zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]) {
                Vertex::fixed(*c, uv, brightness, false).push(&mut b.vertices);
            }
            b.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    b.finish(RgbaImage { width: W, height: H, texels })
}

/// Which optional parts of the scene to draw (the options screen's Land,
/// Sea and Sky).
#[derive(Clone, Copy)]
pub struct Show {
    pub land: bool,
    pub sea: bool,
    pub sky: bool,
}

/// Builds everything drawn for level `n`.
pub fn build(data: &mut GameData, n: u32, show: Show) -> Result<BuiltLevel, l3d_formats::Error> {
    let level = data.level(n)?;
    let blocks = data.blocks(n)?;
    let pal = data.palette("GFX/LM3D.PAL")?;
    let mesh = level_mesh::build(&level, &blocks);
    let mut scene = SceneData::default();

    let tex = data.gfx("TEXTURE", level.texture_set)?;
    let flags = level.flags;
    let surround = flags & level_flags::SURROUND_SKY != 0;

    if show.sea
        && level.sea_gfx != 0xFF
        && !surround
        && flags & level_flags::SEA_SOLID_COLOUR == 0
        && let Ok(sea) = data.gfx("SEA", level.sea_gfx)
    {
        // With the "128×128" flag the 16 KB file is one 128×128 texture;
        // otherwise it is a 64-wide strip of 64×64 frames, which animate
        // unless the level says the sea is still. Texel density matches block
        // faces, 64 per grid unit (unverified).
        let (img, tile, size, frames) = if flags & level_flags::SEA_128 != 0 {
            (indexed_to_rgba(&sea[..(128 * 128).min(sea.len())], 128, &pal), 2.0, 128.0, 1)
        } else {
            let frames = if flags & level_flags::SEA_STILL != 0 { 1 } else { (sea.len() / (64 * 64)).max(1) as u32 };
            (indexed_to_rgba(&sea[..(64 * 64 * frames as usize).min(sea.len())], 64, &pal), 1.0, 64.0, frames)
        };
        // The header's water speeds, read as texels per tick (unverified).
        let per_second = l3d_sim::TICKS_PER_SECOND as f32 / size;
        let drift = [level.water_speed_x as f32 * per_second, level.water_speed_z as f32 * per_second];
        scene.layers.push(plane_layer(GROUND_Y - 0.01, tile, img, frames, drift));
    }
    if show.land
        && level.land_gfx != 0xFF
        && flags & level_flags::LAND_INVISIBLE == 0
        && !level.land_polygons.is_empty()
        && let Ok(land) = data.gfx("LAND", level.land_gfx)
    {
        // With the "128" flag the file is one 128×128 texture spanning 4 grid
        // units (verified visually on Mayhem 6 and Fun 1); without it, a strip
        // of 64×64 textures, each spanning 2 units, chosen per polygon
        // (`docs/spec/graphics.md`).
        let (size, mut tile) = if flags & level_flags::LAND_128 != 0 { (128, 4.0) } else { (64, 2.0) };
        if flags & level_flags::LAND_HALF_SCALE != 0 {
            tile /= 2.0;
        }
        let count = (land.len() / (size * size)).max(1);
        for index in 0..count {
            let polys: Vec<_> = level.land_polygons.iter().filter(|p| land_texture(p) % count == index).collect();
            if polys.is_empty() {
                continue;
            }
            let pixels = &land[index * size * size..((index + 1) * size * size).min(land.len())];
            scene.layers.push(land_layer(&polys, indexed_to_rgba(pixels, size as u32, &pal), tile));
        }
    }
    let block_layer_index = scene.layers.len();
    scene.layers.push(block_layer(&mesh, indexed_to_rgba(&tex, 64, &pal)));
    // Builders' bricks, right after the blocks (see ebuild_blocks).
    scene.layers.push(brick_layer(&[]));
    scene.layers.extend(object_layers(data, &level, &pal));
    scene.layers.extend(decal_layers(data, &level, &pal));
    scene.layers.extend(trap_layer(data, &level, &pal));

    let traps = (level.trap_type != 0xFF).then(|| data.gfx("TRAPS", level.trap_type).ok()).flatten();
    scene.sprite_atlas = lemming_atlas(data, &pal, traps.as_deref()).ok();

    if show.sky
        && level.sky_gfx != 0xFF
        && let Ok(sky) = data.gfx("SKY", level.sky_gfx)
    {
        match sky.len() {
            // 1024×64 panorama above the horizon (verified).
            65536 => scene.sky = Some(indexed_to_rgba(&sky, 1024, &pal)),
            // All-round sky stored as 320 rows of 200 pixels: each stored row
            // is one screen column, so the image is transposed into a
            // 320×200 panorama that wraps horizontally (its stored top and
            // bottom rows join seamlessly; its sides don't). Verified from
            // the data; how the original scales and pans it is provisional.
            64000 => {
                let src = &sky;
                let transposed: Vec<u8> = (0..200).flat_map(|y| (0..320).map(move |x| src[x * 200 + y])).collect();
                scene.sky = Some(indexed_to_rgba(&transposed, 320, &pal));
                scene.sky_surround = true;
            }
            _ => {}
        }
    }
    Ok(BuiltLevel { level, blocks, scene, mesh, block_layer: block_layer_index })
}
