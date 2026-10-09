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
    /// Header byte `0x157`: bit 0, the level bottom is slippery; bit 1, the
    /// land is.
    slippery_floor: u8,
    /// Cells whose block changed since the last [`World::take_changes`], with
    /// their new contents (segments 0 = removed).
    changes: Vec<([usize; 3], BlockCell)>,
    /// Builders' bricks, which lie across cell boundaries.
    pub bricks: Vec<Brick>,
    /// Set when [World::bricks] changed since the last
    /// [World::take_bricks_changed].
    bricks_changed: bool,
    /// The level's most common ordinary block id (≥ 9), used for terrain
    /// that lemmings create where no block gives a better choice.
    pub common_id: u8,
}

/// A builder's brick: a box from `min` (inclusive) to `max` (exclusive)
/// in sub-units, showing block `id`'s texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brick {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub id: u8,
}

impl Brick {
    pub fn contains(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| self.min[i] <= p[i] && p[i] < self.max[i])
    }
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
            cells[index(x, y, z)] = Some(SolidCell {
                block: b,
                flags: def.flags,
            });
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
        let common_id = (9..64)
            .max_by_key(|&i| (counts[i], std::cmp::Reverse(i)))
            .unwrap_or(9) as u8;
        World {
            cells,
            land,
            bottom_solid: level.flags & FLAG_BOTTOM_SOLID != 0,
            slippery_floor: level.flags2 & 3,
            changes: Vec::new(),
            bricks: Vec::new(),
            bricks_changed: false,
            common_id,
        }
    }

    /// Removes the segments in `mask` from cell `c`. Steel blocks are
    /// unaffected. Returns whether anything was removed.
    pub fn remove_segments(&mut self, c: [i32; 3], mask: u8) -> bool {
        self.remove_segments_towards(c, mask, None)
    }

    /// Like [`World::remove_segments`], for a lemming digging horizontally in
    /// direction `toward` (a unit step), or not horizontally (`None`). One-way
    /// blocks (ids 5–8) give way only in their direction: +Z, −X, −Z, +X
    /// for ids 5–8 before rotation (the way their arrows point), turned with
    /// the block's rotation like its faces. Non-horizontal digging never
    /// breaks them (provisional).
    /// Bricks overlapping the removed segments go too.
    pub fn remove_segments_towards(
        &mut self,
        c: [i32; 3],
        mask: u8,
        toward: Option<[i32; 3]>,
    ) -> bool {
        let bricks_removed = self.remove_bricks(c, mask);
        let Some(mut cell) = self.cell(c[0], c[1], c[2]) else {
            return bricks_removed;
        };
        if cell.block.segments & mask == 0 || !self.breakable_towards(c, toward) {
            return bricks_removed;
        }
        cell.block.segments &= !mask;
        let i = index(c[0] as usize, c[1] as usize, c[2] as usize);
        self.cells[i] = (cell.block.segments != 0).then_some(cell);
        self.changes.push((c.map(|v| v as usize), cell.block));
        true
    }

    /// Whether the terrain block in cell `c` gives way to digging in
    /// direction `toward` (as for [`World::remove_segments_towards`]): not
    /// steel, and not a one-way block facing elsewhere. Empty cells do.
    pub fn breakable_towards(&self, c: [i32; 3], toward: Option<[i32; 3]>) -> bool {
        self.cell(c[0], c[1], c[2]).is_none_or(|cell| {
            cell.flags & flags::STEEL == 0
                && (!(5..=8).contains(&cell.block.id)
                    || toward == Some(one_way_direction(cell.block)))
        })
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
            block: BlockCell {
                id: template.id,
                shape: 0,
                rotation: 0,
                segments: 0,
            },
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

    /// Removes the bricks overlapping the segments in `mask` of cell `c`;
    /// returns whether there were any.
    fn remove_bricks(&mut self, c: [i32; 3], mask: u8) -> bool {
        let before = self.bricks.len();
        let quarter = SUB / 4;
        self.bricks.retain(|b| {
            !(0..4).any(|seg| {
                let min = [c[0] * SUB, c[1] * SUB + seg * quarter, c[2] * SUB];
                let max = [min[0] + SUB, min[1] + quarter, min[2] + SUB];
                mask & (1 << seg) != 0 && (0..3).all(|i| b.min[i] < max[i] && min[i] < b.max[i])
            })
        });
        let removed = self.bricks.len() != before;
        self.bricks_changed |= removed;
        removed
    }

    /// Adds a brick unless it would overlap solid terrain (sampled at its
    /// corners and centre); returns whether it was added.
    pub fn add_brick(&mut self, brick: Brick) -> bool {
        let (lo, hi) = (brick.min, brick.max.map(|v| v - 1));
        let mid = [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2);
        let samples = [
            [lo[0], mid[1], lo[2]],
            [hi[0], mid[1], lo[2]],
            [lo[0], mid[1], hi[2]],
            [hi[0], mid[1], hi[2]],
            mid,
        ];
        if samples.iter().any(|&p| self.solid(p)) {
            return false;
        }
        self.bricks.push(brick);
        self.bricks_changed = true;
        true
    }

    /// Whether the bricks changed since the last call.
    pub fn take_bricks_changed(&mut self) -> bool {
        std::mem::take(&mut self.bricks_changed)
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

    /// Whether the point (in sub-units) is inside solid block geometry or a
    /// brick.
    pub fn solid(&self, p: [i32; 3]) -> bool {
        self.bricks.iter().any(|b| b.contains(p)) || self.terrain_solid(p)
    }

    /// Whether the point is inside solid block geometry, bricks aside.
    pub fn terrain_solid(&self, p: [i32; 3]) -> bool {
        let c = p.map(|v| v.div_euclid(SUB));
        let Some(cell) = self.cell(c[0], c[1], c[2]) else {
            return false;
        };
        let l = [0, 1, 2].map(|i| (p[i] - c[i] * SUB) as f32 / SUB as f32);
        inside_shape(cell.block, l)
    }

    /// For a point inside a 45° deflector wall (shape 7): the outward normal
    /// of its diagonal face, as `[x, z]` signs (each ±1).
    pub fn deflector_normal(&self, p: [i32; 3]) -> Option<[i32; 2]> {
        const DEFLECTOR: u8 = 7;
        let c = p.map(|v| v.div_euclid(SUB));
        let cell = self.cell(c[0], c[1], c[2])?;
        if cell.block.shape != DEFLECTOR || !self.solid(p) {
            return None;
        }
        // The diagonal that has solid on one side of the centre and open
        // space on the other, whatever the rotation.
        let ly = (p[1] - c[1] * SUB) as f32 / SUB as f32;
        [[1, 1], [1, -1], [-1, 1], [-1, -1]].into_iter().find(|n| {
            let at = |s: f32| [0.5 + s * n[0] as f32, ly, 0.5 + s * n[1] as f32];
            !inside_shape(cell.block, at(0.3)) && inside_shape(cell.block, at(-0.3))
        })
    }

    /// The block whose geometry contains the point, if any.
    pub fn block_at(&self, p: [i32; 3]) -> Option<BlockCell> {
        let c = p.map(|v| v.div_euclid(SUB));
        self.solid(p)
            .then(|| self.cell(c[0], c[1], c[2]).map(|s| s.block))
            .flatten()
    }

    /// Block flags at a point, if it is inside solid geometry.
    pub fn flags_at(&self, p: [i32; 3]) -> Option<u8> {
        let c = p.map(|v| v.div_euclid(SUB));
        self.solid(p)
            .then(|| self.cell(c[0], c[1], c[2]).map(|s| s.flags))
            .flatten()
    }

    /// Whether the ground plane at column `(x, z)` (sub-units) is slippery:
    /// on land by header bit 1, elsewhere by bit 0 ([L3DEdit]; inferred: only
    /// the ice levels set them, and Fun 14 "Slippery Maze" needs its
    /// floor slippery).
    pub fn floor_slippery(&self, x: i32, z: i32) -> bool {
        let (px, pz) = (x as f32 / SUB as f32, z as f32 / SUB as f32);
        let on_land = self.land.iter().any(|poly| point_in_convex(poly, px, pz));
        let bit = if on_land { 2 } else { 1 };
        self.slippery_floor & bit != 0
    }

    /// What the ground plane is at column `(x, z)` (sub-units).
    pub fn floor(&self, x: i32, z: i32) -> Floor {
        if self.bottom_solid {
            return Floor::Land;
        }
        let (px, pz) = (x as f32 / SUB as f32, z as f32 / SUB as f32);
        if self.land.iter().any(|poly| point_in_convex(poly, px, pz)) {
            Floor::Land
        } else {
            Floor::Water
        }
    }

    /// The height of the highest walkable surface at column `(x, z)` whose top
    /// is at or below `from_y`, searching down to the ground plane. Returns the
    /// surface height in sub-units and whether it is a block (as opposed to the
    /// ground plane). Bricks are floors only: one counts by its top, when
    /// that is at or below `from_y`; a brick higher up is passed under.
    pub fn surface_below(&self, x: i32, from_y: i32, z: i32) -> (i32, bool) {
        let terrain = self.terrain_surface_below(x, from_y, z);
        let brick = self
            .bricks
            .iter()
            .filter(|b| {
                (b.min[0]..b.max[0]).contains(&x)
                    && (b.min[2]..b.max[2]).contains(&z)
                    && b.max[1] <= from_y
            })
            .map(|b| b.max[1])
            .max();
        match brick {
            Some(top) if top > terrain.0 => (top, true),
            _ => terrain,
        }
    }

    fn terrain_surface_below(&self, x: i32, from_y: i32, z: i32) -> (i32, bool) {
        let mut y = from_y;
        // Step down in fine increments until the point below is solid.
        const STEP: i32 = SUB / 64;
        while y > GROUND {
            if self.terrain_solid([x, y - 1, z]) {
                return (y, true);
            }
            y -= STEP;
            // Snap to the precise surface when we enter solid geometry.
            if self.terrain_solid([x, y - 1, z]) {
                let mut top = y;
                while self.terrain_solid([x, top, z]) {
                    top += 1;
                }
                return (top, true);
            }
        }
        // The steps can pass over terrain within a step of the ground, such
        // as the foot of a ramp (Mayhem 2 "Brechin's Staircase").
        if self.terrain_solid([x, GROUND, z]) {
            let mut top = GROUND;
            while self.terrain_solid([x, top, z]) {
                top += 1;
            }
            return (top, true);
        }
        (GROUND, false)
    }
}

/// The direction a one-way block (ids 5–8) can be dug through: +Z, −X, −Z,
/// +X before rotation (the way its arrows point), each rotation step turning
/// +X → −Z → −X → +Z (the renderer's block-rotation sense).
fn one_way_direction(block: BlockCell) -> [i32; 3] {
    let mut d = match block.id {
        5 => [0, 0, 1],
        6 => [-1, 0, 0],
        7 => [0, 0, -1],
        _ => [1, 0, 0],
    };
    for _ in 0..block.rotation % 4 {
        // (x, z) → (z, −x): +X → −Z.
        d = [d[2], 0, -d[0]];
    }
    d
}

fn index(x: usize, y: usize, z: usize) -> usize {
    (x * SIZE_Y + y) * SIZE_Z + z
}

/// Convex polygon containment, independent of winding.
fn point_in_convex(poly: &[(i32, i32)], x: f32, z: f32) -> bool {
    let mut sign = 0.0f32;
    for i in 0..poly.len() {
        let (ax, az) = (poly[i].0 as f32, poly[i].1 as f32);
        let (bx, bz) = (
            poly[(i + 1) % poly.len()].0 as f32,
            poly[(i + 1) % poly.len()].1 as f32,
        );
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

/// The thickness of a ramp at its thin edge (22.5° ramps, and the flat top
/// of an upside-down 45° ramp), in units.
const RAMP_EDGE: f32 = 1.0 / 32.0;

/// Whether the cell-local point `l` (each in `[0, 1)`) lies inside the block.
/// Shapes mirror the renderer's (`docs/spec/level.md`); the vertical extent is
/// the run of consecutive segments at the point's height (a cell a basher has
/// cut through the middle has two).
pub fn inside_shape(block: BlockCell, l: [f32; 3]) -> bool {
    let s = block.segments;
    let at = ((l[1] * 4.0).floor() as i32).clamp(0, 3);
    if s & (1 << at) == 0 {
        return false;
    }
    let (mut lo, mut hi) = (at, at + 1);
    while lo > 0 && s & (1 << (lo - 1)) != 0 {
        lo -= 1;
    }
    while hi < 4 && s & (1 << hi) != 0 {
        hi += 1;
    }
    let (lo, hi) = (lo as f32 / 4.0, hi as f32 / 4.0);
    if l[1] < lo || l[1] >= hi {
        return false;
    }
    // Outer-corner pieces (10, 11) carry one extra quarter turn, matching the
    // renderer (`level_mesh::effective_rotation`).
    let rotation = (block.rotation + if matches!(block.shape, 10 | 11) { 1 } else { 0 }) % 4;
    let [x, y, z] = to_shape_space(l, rotation);
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
        // A turner standing on the thin edge of the flat top (Taxing 5) must
        // not fall through it.
        6 => y > (lo + (hi - lo) * z).min(hi - RAMP_EDGE),
        7 => x + z < 1.0,
        // 22.5° ramps keep their slope whatever the run's height: the top
        // falls ½ unit across the cell (or the run's height, if less) from
        // the run's top, or for 9 the underside rises as far from its bottom,
        // so a half-height ramp cell and a full one make one even slope (the
        // ramp tubes of Taxing 1 "Spaghetti Junction"). Matches the renderer,
        // but for a sliver at the thin edge so that a ramp meeting another at
        // the cell boundary leaves no gap there.
        8 => y < (hi - (hi - lo).min(0.5) * z).max(lo + RAMP_EDGE),
        9 => y > (lo + (hi - lo).min(0.5) * z).min(hi - RAMP_EDGE),
        10 => t < x.min(z),
        11 => 1.0 - t < x.min(z),
        12 => x + z <= 1.0 && t <= 1.0 - x - z,
        13 => x + z <= 1.0 && 1.0 - t <= 1.0 - x - z,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(shape: u8, rotation: u8, segments: u8) -> BlockCell {
        BlockCell {
            id: 9,
            shape,
            rotation,
            segments,
        }
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
