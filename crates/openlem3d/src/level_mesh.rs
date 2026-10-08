//! Converts a level's block grid into triangle geometry. Engine-independent:
//! produces plain vertex arrays.
//!
//! World space equals grid space: cell `(x, y, z)` occupies the unit cube
//! `[x, x+1] × [y, y+1] × [z, z+1]`.

use l3d_formats::blk::{BlockSet, FaceDir, flags, modifiers};
use l3d_formats::level::{BlockCell, Level, SIZE_X, SIZE_Y, SIZE_Z};

/// Number of 64×64 tiles in a `TEXTURE` file.
pub const TILES: u32 = 100;

/// Triangle geometry with per-vertex position, texture coordinate and
/// brightness. `uv` addresses the whole 64×6400 texture strip.
#[derive(Default, Debug)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    /// Per-vertex texture animation: frame count (negative for a
    /// ping-pong) and the uv step between frames; `[1, 0]` when still.
    pub anims: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Geometry split by how it must be drawn.
#[derive(Default, Debug)]
pub struct LevelMesh {
    /// Faces without transparency.
    pub opaque: MeshData,
    /// Faces whose palette index 0 is transparent.
    pub cutout: MeshData,
    /// Cells whose shape is not modelled yet, as `(shape, count)`.
    pub unsupported_shapes: Vec<(u8, usize)>,
}

/// A planar polygon of a shape, in unit-cube local coordinates.
struct Poly {
    verts: Vec<[f32; 3]>,
    /// The block-definition face whose texture this polygon shows.
    src: FaceDir,
    /// For polygons lying on a cube face: the direction of that face, so it can
    /// be culled against a solid neighbour.
    on_face: Option<FaceDir>,
}

/// A shape polygon placed in world space: vertices, tile texture
/// coordinates, and the source polygon.
type PlacedPoly<'a> = (Vec<[f32; 3]>, Vec<[f32; 2]>, &'a Poly, bool);

fn poly(verts: &[[f32; 3]], src: FaceDir, on_face: Option<FaceDir>) -> Poly {
    Poly { verts: verts.to_vec(), src, on_face }
}

fn mirror_y(p: Poly) -> Poly {
    let flip = |d: FaceDir| match d {
        FaceDir::PosY => FaceDir::NegY,
        FaceDir::NegY => FaceDir::PosY,
        d => d,
    };
    Poly {
        verts: p.verts.iter().map(|v| [v[0], 1.0 - v[1], v[2]]).collect(),
        src: flip(p.src),
        on_face: p.on_face.map(flip),
    }
}

/// Polygons of a shape at rotation 0. Shapes follow `docs/spec/level.md`
/// (descriptions from [L3DEdit], unverified); unknown shapes return `None`.
fn shape_polys(shape: u8) -> Option<Vec<Poly>> {
    use FaceDir::*;
    let full = |d: FaceDir| -> Poly {
        let v = match d {
            PosZ => [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
            NegZ => [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            PosX => [[1., 0., 0.], [1., 0., 1.], [1., 1., 1.], [1., 1., 0.]],
            NegX => [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
            PosY => [[0., 1., 0.], [1., 1., 0.], [1., 1., 1.], [0., 1., 1.]],
            NegY => [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        };
        poly(&v, d, Some(d))
    };
    Some(match shape {
        0 => FaceDir::ALL.iter().map(|&d| full(d)).collect(),
        // Square pyramid, apex up; 1 is half height, 3 full height.
        1 | 3 => {
            let h = if shape == 1 { 0.5 } else { 1.0 };
            let a = [0.5, h, 0.5];
            vec![
                full(NegY),
                poly(&[[0., 0., 1.], [1., 0., 1.], a], PosZ, None),
                poly(&[[0., 0., 0.], [1., 0., 0.], a], NegZ, None),
                poly(&[[1., 0., 0.], [1., 0., 1.], a], PosX, None),
                poly(&[[0., 0., 0.], [0., 0., 1.], a], NegX, None),
            ]
        }
        2 | 4 => shape_polys(shape - 1)?.into_iter().map(mirror_y).collect(),
        // 45° ramp rising towards −Z (surface faces +Z/+Y).
        5 => vec![
            full(NegY),
            full(NegZ),
            poly(&[[0., 0., 0.], [0., 0., 1.], [0., 1., 0.]], NegX, Some(NegX)),
            poly(&[[1., 0., 0.], [1., 0., 1.], [1., 1., 0.]], PosX, Some(PosX)),
            poly(&[[0., 0., 1.], [1., 0., 1.], [1., 1., 0.], [0., 1., 0.]], PosZ, None),
        ],
        6 => shape_polys(5)?.into_iter().map(mirror_y).collect(),
        // Vertical 45° deflector, diagonal face towards +Z/+X.
        7 => vec![
            poly(&[[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]], NegY, Some(NegY)),
            poly(&[[0., 1., 0.], [1., 1., 0.], [0., 1., 1.]], PosY, Some(PosY)),
            full(NegZ),
            full(NegX),
            poly(&[[1., 0., 0.], [0., 0., 1.], [0., 1., 1.], [1., 1., 0.]], PosZ, None),
        ],
        // 22.5° ramp over the upper half, falling towards +Z.
        8 => vec![
            full(NegY),
            full(NegZ),
            poly(&[[0., 0., 0.], [0., 0., 1.], [0., 0.5, 1.], [0., 1., 0.]], NegX, Some(NegX)),
            poly(&[[1., 0., 0.], [1., 0., 1.], [1., 0.5, 1.], [1., 1., 0.]], PosX, Some(PosX)),
            poly(&[[0., 0., 1.], [1., 0., 1.], [1., 0.5, 1.], [0., 0.5, 1.]], PosZ, Some(PosZ)),
            poly(&[[0., 0.5, 1.], [1., 0.5, 1.], [1., 1., 0.], [0., 1., 0.]], PosY, None),
        ],
        9 => shape_polys(8)?.into_iter().map(mirror_y).collect(),
        // Outer corner of two 45° ramps rising towards +X and +Z.
        10 => vec![
            full(NegY),
            poly(&[[0., 0., 0.], [1., 0., 0.], [1., 1., 1.]], NegZ, None),
            poly(&[[0., 0., 0.], [0., 0., 1.], [1., 1., 1.]], NegX, None),
            poly(&[[1., 0., 0.], [1., 0., 1.], [1., 1., 1.]], PosX, Some(PosX)),
            poly(&[[0., 0., 1.], [1., 0., 1.], [1., 1., 1.]], PosZ, Some(PosZ)),
        ],
        11 => shape_polys(10)?.into_iter().map(mirror_y).collect(),
        // Corner piece with one sloped face towards −X/−Z/+Y.
        12 => vec![
            poly(&[[1., 0., 0.], [1., 0., 1.], [0., 0., 1.]], NegY, Some(NegY)),
            poly(&[[1., 0., 0.], [0., 0., 1.], [1., 1., 1.]], PosZ, None),
            poly(&[[1., 0., 0.], [1., 0., 1.], [1., 1., 1.]], PosX, Some(PosX)),
            poly(&[[0., 0., 1.], [1., 0., 1.], [1., 1., 1.]], PosZ, Some(PosZ)),
        ],
        13 => shape_polys(12)?.into_iter().map(mirror_y).collect(),
        _ => return None,
    })
}

/// Rotates a horizontal direction by `r` quarter turns anticlockwise seen
/// from above (+X → −Z → −X → +Z).
fn rotate_dir(d: FaceDir, r: u8) -> FaceDir {
    use FaceDir::*;
    let mut d = d;
    for _ in 0..r % 4 {
        d = match d {
            PosX => NegZ,
            NegZ => NegX,
            NegX => PosZ,
            PosZ => PosX,
            v => v,
        };
    }
    d
}

/// Whether polygon `src` of `cell` carries an entrance hatch's flap picture:
/// the local ±X side faces of an entrance block (id 0). The hatch's bottom
/// opens as two doors hinged at those walls' bottom edges; the walls stay.
fn is_flap(cell: BlockCell, src: FaceDir) -> bool {
    cell.id == 0 && cell.shape == 0 && matches!(src, FaceDir::PosX | FaceDir::NegX)
}

/// Where a point of a hatch side face lies on the door hinged at that
/// wall's bottom edge, `open` (0–1) of the way open. Closed, the door lies
/// flat across its half of the hatch's bottom; it swings down through 135°
/// until it hangs down and out at 45° (matches the original's open hatches;
/// angle estimated from captures).
fn open_flap(p: [f32; 3], src: FaceDir, y0: f32, open: f32) -> [f32; 3] {
    let a = open.clamp(0.0, 1.0) * 0.75 * std::f32::consts::PI;
    let h = p[1] - y0;
    let x = if src == FaceDir::PosX { 1.0 - a.cos() * h } else { a.cos() * h };
    [x, y0 - a.sin() * h, p[2]]
}

/// The quarter turns actually applied to a cell: the stored rotation plus a
/// per-shape correction to our base shapes. The outer-corner pieces (10, 11)
/// need one extra quarter turn to match the original (verified visually on
/// the pyramids of `LEVEL.029`).
pub fn effective_rotation(shape: u8, rotation: u8) -> u8 {
    let correction = if matches!(shape, 10 | 11) { 1 } else { 0 };
    (rotation + correction) % 4
}

/// Rotates a local point about the cell's vertical centre line, matching
/// [`rotate_dir`].
fn rotate_point(p: [f32; 3], r: u8) -> [f32; 3] {
    let (mut x, mut z) = (p[0] - 0.5, p[2] - 0.5);
    for _ in 0..r % 4 {
        // +X (1,0) → −Z (0,−1): (x, z) → (z, −x).
        (x, z) = (z, -x);
    }
    [x + 0.5, p[1], z + 0.5]
}

fn dir_offset(d: FaceDir) -> [i32; 3] {
    match d {
        FaceDir::PosX => [1, 0, 0],
        FaceDir::NegX => [-1, 0, 0],
        FaceDir::PosY => [0, 1, 0],
        FaceDir::NegY => [0, -1, 0],
        FaceDir::PosZ => [0, 0, 1],
        FaceDir::NegZ => [0, 0, -1],
    }
}

/// Texture coordinates within a tile for a local point, projected onto the
/// source face's plane. `v` runs down the tile.
fn tile_uv(src: FaceDir, p: [f32; 3]) -> [f32; 2] {
    let [x, y, z] = p;
    match src {
        FaceDir::PosZ => [x, 1.0 - y],
        FaceDir::NegZ => [1.0 - x, 1.0 - y],
        FaceDir::PosX => [1.0 - z, 1.0 - y],
        FaceDir::NegX => [z, 1.0 - y],
        FaceDir::PosY => [x, z],
        FaceDir::NegY => [x, 1.0 - z],
    }
}

/// The tiles shown on the front and back of a face. With the reverse-side
/// modifier, texture values 0x08–0x13 select tiles 43–54 and the back shows
/// the paired tile (43↔44, 45↔46, …); 0x14–0x16 show tile 0 ([L3DEdit];
/// verified visually for the gantry in `LEVEL.050`).
/// A face's texture animation: the first tile and the frame count,
/// negative for a ping-pong. Modifiers 0x10 and 0x20 ping-pong over the face's
/// tile and the next 4 or 3; 0x40 loops a run chosen by the tile value
/// ([L3DEdit] BLK notes, from testing in the original; unverified here).
/// Frame rate unknown.
fn face_animation(texture: u8, mods: u8) -> Option<(u32, i32)> {
    if mods == 0xFF {
        return None;
    }
    if mods & modifiers::ANIMATE_4 != 0 {
        return Some((texture as u32, -4));
    }
    if mods & modifiers::ANIMATE_5 != 0 {
        return Some((texture as u32, -5));
    }
    if mods & modifiers::ANIMATE_SPECIAL != 0 {
        return match texture {
            0 => Some((12, 4)),
            1 => Some((16, 4)),
            2 => Some((0, 8)),
            3 => Some((28, 4)),
            4 => Some((32, 4)),
            5 => Some((36, 4)),
            6 => Some((0, 4)),
            _ => None,
        };
    }
    None
}

fn face_tiles(texture: u8, mods: u8) -> (u32, u32) {
    if mods & modifiers::REVERSE_SIDE != 0 {
        match texture {
            0x08..=0x13 => {
                let front = 43 + (texture - 0x08) as u32;
                let back = if (front - 43).is_multiple_of(2) { front + 1 } else { front - 1 };
                return (front, back);
            }
            0x14..=0x16 => return (0, 0),
            _ => {}
        }
    }
    (texture as u32, texture as u32)
}

/// Block ids with hard-coded invisible behaviour (see `docs/spec/blk.md`).
const INVISIBLE_IDS: [u8; 2] = [3, 4];

/// Vertical extent `(bottom, top)` within the cell, from the segment mask.
fn segment_span(segments: u8) -> (f32, f32) {
    let lo = segments.trailing_zeros() as f32;
    let hi = 8.0 - segments.leading_zeros() as f32; // index of highest set bit + 1
    (lo / 4.0, hi / 4.0)
}

struct Grid<'a> {
    level: &'a Level,
    blocks: &'a BlockSet,
}

impl Grid<'_> {
    fn cell(&self, p: [i32; 3]) -> Option<BlockCell> {
        let in_range = |v: i32, n: usize| v >= 0 && (v as usize) < n;
        (in_range(p[0], SIZE_X) && in_range(p[1], SIZE_Y) && in_range(p[2], SIZE_Z))
            .then(|| self.level.block(p[0] as usize, p[1] as usize, p[2] as usize))
    }

    /// True if the cell is an opaque full cube, or the face-suppressing block 3.
    fn hides_neighbour_faces(&self, p: [i32; 3]) -> bool {
        let Some(c) = self.cell(p) else { return false };
        if c.is_empty() {
            return false;
        }
        if c.id == 3 {
            return true;
        }
        let def = &self.blocks.defs[c.id as usize];
        c.shape == 0
            && c.segments == 0xF
            && !INVISIBLE_IDS.contains(&c.id)
            && !def.is_placeholder()
            && def.flags & flags::DOUBLE_SIDED == 0
            && def.faces.iter().all(|f| f.modifiers & (modifiers::COLOR0_TRANSPARENT | modifiers::REVERSE_SIDE) == 0)
    }
}

impl MeshData {
    fn push_poly(&mut self, verts: &[[f32; 3]], uvs: &[[f32; 2]], brightness: f32, both_sides: bool, anim: [f32; 2]) {
        let n = {
            let (a, b, c) = (verts[0], verts[1], verts[2]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
            n.map(|c| c / len)
        };
        let color = [brightness, brightness, brightness, 1.0];
        for side in 0..if both_sides { 2 } else { 1 } {
            let base = self.positions.len() as u32;
            let normal = if side == 0 { n } else { n.map(|c| -c) };
            for (p, uv) in verts.iter().zip(uvs) {
                self.positions.push(*p);
                self.normals.push(normal);
                self.uvs.push(*uv);
                self.colors.push(color);
                self.anims.push(anim);
            }
            for i in 1..verts.len() as u32 - 1 {
                if side == 0 {
                    self.indices.extend([base, base + i, base + i + 1]);
                } else {
                    self.indices.extend([base, base + i + 1, base + i]);
                }
            }
        }
    }
}

/// Builds the geometry of all visible blocks.
/// `art_on_top(tile)` says whether a tile's picture is in its top half; a
/// half-height hatch's flaps show the half that has it (texture sets differ:
/// `TEXTURE.004` and `.006` draw the crate side in the top half of tiles 59
/// and 60). `hatch_open` (0–1) is how far the hatches' doors are open.
pub fn build(level: &Level, blocks: &BlockSet, art_on_top: &dyn Fn(u32) -> bool, hatch_open: f32) -> LevelMesh {
    let grid = Grid { level, blocks };
    let mut out = LevelMesh::default();
    let mut unsupported = std::collections::BTreeMap::<u8, usize>::new();
    for (x, y, z, cell, _) in level.cells() {
        if cell.is_empty() || INVISIBLE_IDS.contains(&cell.id) {
            continue;
        }
        let def = &blocks.defs[cell.id as usize];
        if def.is_placeholder() {
            continue;
        }
        let Some(polys) = shape_polys(cell.shape) else {
            *unsupported.entry(cell.shape).or_default() += 1;
            continue;
        };
        let (y0, y1) = segment_span(cell.segments);
        let origin = [x as f32, y as f32, z as f32];
        let mut solid_verts: Vec<[f32; 3]> = Vec::new();
        let mut placed: Vec<PlacedPoly> = Vec::new();
        // A hatch keeps all its walls; each flap is a second copy of a side
        // wall, swung open below it.
        let flaps = polys.iter().filter(|p| is_flap(cell, p.src)).map(|p| (p, true));
        for (p, flap) in polys.iter().map(|p| (p, false)).chain(flaps) {
            let uvs: Vec<[f32; 2]> = p
                .verts
                .iter()
                .map(|v| {
                    // Faces show the part of their tile matching their height;
                    // flaps show its bottom part, or the top (below) when the
                    // art is there.
                    // A flap is upside down once open (its top edge swings out and
                    // down), so its picture is flipped to keep the hinges at the
                    // hinge.
                    let ly = if flap { (1.0 - v[1]) * (y1 - y0) } else { y0 + v[1] * (y1 - y0) };
                    tile_uv(p.src, [v[0], ly, v[2]])
                })
                .collect();
            let verts: Vec<[f32; 3]> = p
                .verts
                .iter()
                .map(|v| {
                    let mut lp = [v[0], y0 + v[1] * (y1 - y0), v[2]];
                    if flap {
                        lp = open_flap(lp, p.src, y0, hatch_open);
                    }
                    let r = rotate_point(lp, effective_rotation(cell.shape, cell.rotation));
                    [r[0] + origin[0], r[1] + origin[1], r[2] + origin[2]]
                })
                .collect();
            solid_verts.extend(&verts);
            placed.push((verts, uvs, p, flap));
        }
        let centre = {
            let n = solid_verts.len() as f32;
            let s = solid_verts.iter().fold([0.0; 3], |a, v| [a[0] + v[0], a[1] + v[1], a[2] + v[2]]);
            s.map(|c| c / n)
        };
        for (mut verts, mut uvs, p, flap) in placed {
            // Faces on the cell boundary are hidden by an opaque neighbour.
            if let Some(local) = p.on_face.filter(|_| !flap) {
                let world = rotate_dir(local, effective_rotation(cell.shape, cell.rotation));
                let on_boundary = match world {
                    FaceDir::PosY => y1 >= 1.0,
                    FaceDir::NegY => y0 <= 0.0,
                    _ => true,
                };
                let o = dir_offset(world);
                let n = [x as i32 + o[0], y as i32 + o[1], z as i32 + o[2]];
                if on_boundary && grid.hides_neighbour_faces(n) {
                    continue;
                }
            }
            let face = def.face(p.src);
            if face.texture == 0xFF || face.texture as u32 >= TILES {
                continue;
            }
            // Wind the polygon counter-clockwise as seen from outside.
            let (a, b, c) = (verts[0], verts[1], verts[2]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let pc = {
                let k = verts.len() as f32;
                let s = verts.iter().fold([0.0; 3], |acc, q| [acc[0] + q[0], acc[1] + q[1], acc[2] + q[2]]);
                s.map(|c| c / k)
            };
            let out_dir = [pc[0] - centre[0], pc[1] - centre[1], pc[2] - centre[2]];
            if n[0] * out_dir[0] + n[1] * out_dir[1] + n[2] * out_dir[2] < 0.0 {
                verts.reverse();
                uvs.reverse();
            }
            let (mut front, mut back) = face_tiles(face.texture, face.modifiers);
            let anim = match face_animation(face.texture, face.modifiers) {
                Some((first, frames)) => {
                    (front, back) = (first, first);
                    [frames as f32, 1.0 / TILES as f32]
                }
                None => [1.0, 0.0],
            };
            let strip = |tile: u32| -> Vec<[f32; 2]> {
                let shift = if flap && art_on_top(tile) { y1 - y0 - 1.0 } else { 0.0 };
                uvs.iter().map(|[u, v]| [*u, (tile as f32 + (v + shift).clamp(0.0, 1.0)) / TILES as f32]).collect()
            };
            let brightness = 1.0 - (face.shading.min(8) as f32) * 0.07;
            let transparent = face.modifiers & (modifiers::COLOR0_TRANSPARENT | modifiers::REVERSE_SIDE) != 0;
            // An open hatch shows the inside of its walls too.
            let double = def.flags & flags::DOUBLE_SIDED != 0 || flap || (cell.id == 0 && cell.shape == 0);
            let target = if transparent { &mut out.cutout } else { &mut out.opaque };
            target.push_poly(&verts, &strip(front), brightness, false, anim);
            if double {
                // The inside of a double-sided face: same polygon, opposite
                // winding, possibly another tile.
                let (mut rv, mut ru) = (verts.clone(), strip(back));
                rv.reverse();
                ru.reverse();
                target.push_poly(&rv, &ru, brightness, false, anim);
            }
        }
    }
    out.unsupported_shapes = unsupported.into_iter().collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_consistency() {
        // Rotating a face's points must move them onto the rotated face.
        for r in 0..4 {
            for d in [FaceDir::PosX, FaceDir::NegX, FaceDir::PosZ, FaceDir::NegZ] {
                let o = dir_offset(d);
                let p = [0.5 + o[0] as f32 * 0.5, 0.5, 0.5 + o[2] as f32 * 0.5];
                let rp = rotate_point(p, r);
                let ro = dir_offset(rotate_dir(d, r));
                assert_eq!(rp, [0.5 + ro[0] as f32 * 0.5, 0.5, 0.5 + ro[2] as f32 * 0.5]);
            }
        }
    }

    #[test]
    fn segments() {
        assert_eq!(segment_span(0b1111), (0.0, 1.0));
        assert_eq!(segment_span(0b1100), (0.5, 1.0));
        assert_eq!(segment_span(0b0010), (0.25, 0.5));
    }
}
