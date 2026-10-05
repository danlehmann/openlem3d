//! Static collision model of a level: which points are solid to lemmings.
//!
//! Positions are fixed point with [`SUB`] sub-units per grid unit; grid cell
//! `(x, y, z)` spans `[x, x+1) × [y, y+1) × [z, z+1)` units, matching the
//! renderer's world space.

use l3d_formats::blk::{BlockSet, flags};
use l3d_formats::level::{BlockCell, Level, SIZE_X, SIZE_Y, SIZE_Z};

/// Sub-units per grid unit.
pub const SUB: i32 = 256;

/// Height of the ground plane (top of grid layer 0), in sub-units.
pub const GROUND: i32 = SUB;

/// Level header flag: the whole level bottom is solid ground.
const FLAG_BOTTOM_SOLID: u16 = 0x0080;

/// What lies at the ground plane under a column outside any block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Floor {
    /// Walkable land.
    Land,
    /// Water (or another liquid): lemmings drown.
    Water,
}

/// A level's solid geometry for lemmings.
pub struct World {
    cells: Vec<Option<SolidCell>>,
    /// Convex land polygons as `(x, z)` corner points, in grid units.
    land: Vec<Vec<(i32, i32)>>,
    bottom_solid: bool,
    /// Cells whose block changed since the last [`World::take_changes`], with
    /// their new contents (segments 0 = removed).
    changes: Vec<([usize; 3], BlockCell)>,
    /// The level's most common ordinary block id (≥ 9), used for terrain
    /// that lemmings create where no block gives a better choice.
    pub common_id: u8,
}

/// A non-empty cell that blocks lemmings.
#[derive(Debug, Clone, Copy)]
struct SolidCell {
    block: BlockCell,
    /// Behaviour flags of the block definition.
    flags: u8,
}

impl World {
    pub fn new(level: &Level, blocks: &BlockSet) -> Self {
        let mut cells = vec![None; SIZE_X * SIZE_Y * SIZE_Z];
        for (x, y, z, b, _) in level.cells() {
            if b.is_empty() {
                continue;
            }
            let def = &blocks.defs[b.id as usize];
            if def.flags & flags::NON_SOLID_LEMMINGS != 0 {
                continue;
            }
            cells[index(x, y, z)] = Some(SolidCell { block: b, flags: def.flags });
        }
        let land = level
            .land_polygons
            .iter()
            .filter(|p| p.len() >= 3)
            .map(|p| p.iter().map(|v| (v.x as i32, v.z as i32)).collect())
            .collect();
        let mut counts = [0u32; 64];
        for c in cells.iter().flatten() {
            counts[c.block.id as usize] += 1;
        }
        let common_id = (9..64).max_by_key(|&i| (counts[i], std::cmp::Reverse(i))).unwrap_or(9) as u8;
        World { cells, land, bottom_solid: level.flags & FLAG_BOTTOM_SOLID != 0, changes: Vec::new(), common_id }
    }

    /// Removes the segments in `mask` from cell `c`. Steel blocks are
    /// unaffected. Returns whether anything was removed.
    pub fn remove_segments(&mut self, c: [i32; 3], mask: u8) -> bool {
        let Some(mut cell) = self.cell(c[0], c[1], c[2]) else { return false };
        if cell.flags & flags::STEEL != 0 || cell.block.segments & mask == 0 {
            return false;
        }
        cell.block.segments &= !mask;
        let i = index(c[0] as usize, c[1] as usize, c[2] as usize);
        self.cells[i] = (cell.block.segments != 0).then_some(cell);
        self.changes.push((c.map(|v| v as usize), cell.block));
        true
    }

    /// Adds the segments in `mask` to cell `c`. An empty cell becomes a cube
    /// of `template`'s block id; an occupied cell keeps its block. Returns
    /// whether anything was added.
    pub fn add_segments(&mut self, c: [i32; 3], mask: u8, template: BlockCell) -> bool {
        let ok = |v: i32, n: usize| v >= 0 && (v as usize) < n;
        if !(ok(c[0], SIZE_X) && ok(c[1], SIZE_Y) && ok(c[2], SIZE_Z)) {
            return false;
        }
        let i = index(c[0] as usize, c[1] as usize, c[2] as usize);
        let mut cell = self.cells[i].unwrap_or(SolidCell {
            block: BlockCell { id: template.id, shape: 0, rotation: 0, segments: 0 },
            flags: 0,
        });
        if cell.block.segments & mask == mask {
            return false;
        }
        cell.block.segments |= mask;
        self.cells[i] = Some(cell);
        self.changes.push((c.map(|v| v as usize), cell.block));
        true
    }

    /// The solid block in cell `c`, with its flags.
    pub fn block(&self, c: [i32; 3]) -> Option<(BlockCell, u8)> {
        self.cell(c[0], c[1], c[2]).map(|s| (s.block, s.flags))
    }

    /// Drains the list of changed cells.
    pub fn take_changes(&mut self) -> Vec<([usize; 3], BlockCell)> {
        std::mem::take(&mut self.changes)
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<SolidCell> {
        let ok = |v: i32, n: usize| v >= 0 && (v as usize) < n;
        if ok(x, SIZE_X) && ok(y, SIZE_Y) && ok(z, SIZE_Z) {
            self.cells[index(x as usize, y as usize, z as usize)]
        } else {
            None
        }
    }

    /// Whether the point (in sub-units) is inside solid block geometry.
    pub fn solid(&self, p: [i32; 3]) -> bool {
        let c = p.map(|v| v.div_euclid(SUB));
        let Some(cell) = self.cell(c[0], c[1], c[2]) else { return false };
        let l = [0, 1, 2].map(|i| (p[i] - c[i] * SUB) as f32 / SUB as f32);
        inside_shape(cell.block, l)
    }

    /// The block whose geometry contains the point, if any.
    pub fn block_at(&self, p: [i32; 3]) -> Option<BlockCell> {
        let c = p.map(|v| v.div_euclid(SUB));
        self.solid(p).then(|| self.cell(c[0], c[1], c[2]).map(|s| s.block)).flatten()
    }

    /// Block flags at a point, if it is inside solid geometry.
    pub fn flags_at(&self, p: [i32; 3]) -> Option<u8> {
        let c = p.map(|v| v.div_euclid(SUB));
        self.solid(p).then(|| self.cell(c[0], c[1], c[2]).map(|s| s.flags)).flatten()
    }

    /// What the ground plane is at column `(x, z)` (sub-units).
    pub fn floor(&self, x: i32, z: i32) -> Floor {
        if self.bottom_solid {
            return Floor::Land;
        }
        let (px, pz) = (x as f32 / SUB as f32, z as f32 / SUB as f32);
        if self.land.iter().any(|poly| point_in_convex(poly, px, pz)) { Floor::Land } else { Floor::Water }
    }

    /// The height of the highest walkable surface at column `(x, z)` whose top
    /// is at or below `from_y`, searching down to the ground plane. Returns the
    /// surface height in sub-units and whether it is a block (as opposed to the
    /// ground plane).
    pub fn surface_below(&self, x: i32, from_y: i32, z: i32) -> (i32, bool) {
        let mut y = from_y;
        // Step down in fine increments until the point below is solid.
        const STEP: i32 = SUB / 64;
        while y > GROUND {
            if self.solid([x, y - 1, z]) {
                return (y, true);
            }
            y -= STEP;
            // Snap to the precise surface when we enter solid geometry.
            if self.solid([x, y - 1, z]) {
                let mut top = y;
                while self.solid([x, top, z]) {
                    top += 1;
                }
                return (top, true);
            }
        }
        (GROUND, false)
    }
}

fn index(x: usize, y: usize, z: usize) -> usize {
    (x * SIZE_Y + y) * SIZE_Z + z
}

/// Convex polygon containment, independent of winding.
fn point_in_convex(poly: &[(i32, i32)], x: f32, z: f32) -> bool {
    let mut sign = 0.0f32;
    for i in 0..poly.len() {
        let (ax, az) = (poly[i].0 as f32, poly[i].1 as f32);
        let (bx, bz) = (poly[(i + 1) % poly.len()].0 as f32, poly[(i + 1) % poly.len()].1 as f32);
        let cross = (bx - ax) * (z - az) - (bz - az) * (x - ax);
        if cross != 0.0 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

/// Rotates a cell-local horizontal position from world to shape space,
/// inverting the renderer's quarter-turn rotation about the cell centre
/// (`level_mesh::rotate_point`: (x, z) → (z, −x) per step around the centre).
fn to_shape_space(l: [f32; 3], rotation: u8) -> [f32; 3] {
    let (mut x, mut z) = (l[0] - 0.5, l[2] - 0.5);
    for _ in 0..rotation % 4 {
        (x, z) = (-z, x);
    }
    [x + 0.5, l[1], z + 0.5]
}

/// Whether the cell-local point `l` (each in `[0, 1)`) lies inside the block.
/// Shapes mirror the renderer's (`docs/spec/level.md`); the vertical extent is
/// the span of the segment mask.
pub fn inside_shape(block: BlockCell, l: [f32; 3]) -> bool {
    let lo = block.segments.trailing_zeros() as f32 / 4.0;
    let hi = (8 - block.segments.leading_zeros()) as f32 / 4.0;
    if l[1] < lo || l[1] >= hi {
        return false;
    }
    let [x, y, z] = to_shape_space(l, block.rotation);
    let t = (y - lo) / (hi - lo); // height within the block, 0..1
    // Square pyramid profile: 1 at the centre column, 0 at the edges.
    let cone = 1.0 - 2.0 * (x - 0.5).abs().max((z - 0.5).abs());
    match block.shape {
        0 => true,
        1 => t < 0.5 * cone,
        2 => 1.0 - t < 0.5 * cone,
        3 => t < cone,
        4 => 1.0 - t < cone,
        5 => t < 1.0 - z,
        6 => t > z,
        7 => x + z < 1.0,
        8 => t < 1.0 - 0.5 * z,
        9 => 1.0 - t < 1.0 - 0.5 * z,
        10 => t < x.min(z),
        11 => 1.0 - t < x.min(z),
        12 => x + z >= 1.0 && t <= x + z - 1.0,
        13 => x + z >= 1.0 && 1.0 - t <= x + z - 1.0,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(shape: u8, rotation: u8, segments: u8) -> BlockCell {
        BlockCell { id: 9, shape, rotation, segments }
    }

    #[test]
    fn cube_segments() {
        let half = cell(0, 0, 0b0011);
        assert!(inside_shape(half, [0.5, 0.2, 0.5]));
        assert!(!inside_shape(half, [0.5, 0.6, 0.5]));
    }

    #[test]
    fn ramp_rises_towards_neg_z() {
        let r = cell(5, 0, 0xF);
        assert!(inside_shape(r, [0.5, 0.8, 0.1]));
        assert!(!inside_shape(r, [0.5, 0.8, 0.9]));
    }

    #[test]
    fn rotation_matches_renderer() {
        // The renderer maps local +Z faces to world +X after one rotation, so
        // a ramp falling towards +Z falls towards +X once rotated.
        let r = cell(5, 1, 0xF);
        assert!(inside_shape(r, [0.1, 0.8, 0.5]));
        assert!(!inside_shape(r, [0.9, 0.8, 0.5]));
    }
}
