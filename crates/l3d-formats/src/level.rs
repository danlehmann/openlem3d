//! `LEVELS/LEVEL.nnn`: a level's settings, block grid and object grid. See
//! `docs/spec/level.md`.

use crate::Error;

/// Grid extent along X.
pub const SIZE_X: usize = 32;
/// Grid extent along Y (vertical).
pub const SIZE_Y: usize = 16;
/// Grid extent along Z.
pub const SIZE_Z: usize = 32;
pub const CELLS: usize = SIZE_X * SIZE_Y * SIZE_Z;

pub const HEADER_LEN: usize = 0x200;
/// Total size of an unpacked level file.
pub const FILE_LEN: usize = HEADER_LEN + CELLS * 2 * 2;

/// Index of cell `(x, y, z)` in the block and object grids (Z varies fastest,
/// then Y, then X).
pub fn cell_index(x: usize, y: usize, z: usize) -> usize {
    (x * SIZE_Y + y) * SIZE_Z + z
}

/// One cell of the block grid, decoded from its 16-bit value
/// `iiiiii tttt rr ssss` (MSB to LSB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockCell {
    /// Index into the level's block dictionary (0..64).
    pub id: u8,
    /// Shape (0 = cube; see `docs/spec/level.md`).
    pub shape: u8,
    /// Quarter turns about the vertical axis.
    pub rotation: u8,
    /// Bit mask of the four vertical quarter slices that are present; bit 3
    /// is the top slice. Zero means the cell is empty.
    pub segments: u8,
}

impl BlockCell {
    pub fn from_raw(v: u16) -> Self {
        BlockCell {
            id: (v >> 10) as u8,
            shape: ((v >> 6) & 0xF) as u8,
            rotation: ((v >> 4) & 0x3) as u8,
            segments: (v & 0xF) as u8,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segments == 0
    }

    /// The 16-bit grid value of this cell.
    pub fn to_raw(self) -> u16 {
        (self.id as u16) << 10 | (self.shape as u16 & 0xF) << 6 | (self.rotation as u16 & 3) << 4 | (self.segments as u16 & 0xF)
    }
}

/// One cell of the object grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectCell {
    /// Object kind; 0 means no object. Ranges select the graphics file (see spec).
    pub kind: u8,
    /// Second byte; meaning unknown.
    pub extra: u8,
}

/// A vertex of a land polygon; coordinates are grid cell indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LandVertex {
    pub z: u8,
    pub x: u8,
    pub options: u8,
}

/// A preset camera as stored in the header. Positions are signed 8.8 fixed
/// point, relative to grid point (16, 8, 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraPreset {
    pub y: i16,
    pub z: i16,
    pub x: i16,
    /// Facing in quarter turns.
    pub rotation: u8,
}

/// A decoded level file.
#[derive(Debug, Clone)]
pub struct Level {
    /// The raw 512-byte header, including fields not decoded below.
    pub header: Vec<u8>,
    pub time_minutes: u8,
    pub time_seconds: u8,
    /// `(skill, quantity)` pairs.
    pub skills: [(u8, u8); 9],
    /// Up to eight convex land polygons.
    pub land_polygons: Vec<Vec<LandVertex>>,
    pub release_rate: i16,
    pub lemmings: u16,
    pub save_requirement: u16,
    pub texture_set: u8,
    pub land_gfx: u8,
    pub title: String,
    pub object_set: u8,
    pub sign_set: u8,
    pub sea_gfx: u8,
    pub anim_object: u8,
    pub trap_type: u8,
    pub sky_gfx: u8,
    pub walls_set: u8,
    pub water_speed_z: i8,
    pub water_speed_x: i8,
    pub flags: u16,
    pub comment: String,
    pub face_limit: u16,
    pub theme: u8,
    pub music: u8,
    /// `[min_x, min_z, max_x, max_z]`.
    pub border_kill: [u8; 4],
    pub flags2: u8,
    /// `[y, z, x]`.
    pub preview_pivot: [u8; 3],
    pub ceiling_kill: u8,
    /// The four presets in storage order.
    pub cameras: [CameraPreset; 4],
    blocks: Vec<u16>,
    objects: Vec<ObjectCell>,
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

impl Level {
    /// Parses an unpacked level file.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != FILE_LEN {
            return Err(Error::Format(format!("level is {} bytes, expected {FILE_LEN}", data.len())));
        }
        let h = &data[..HEADER_LEN];
        let u16_at = |o: usize| u16::from_le_bytes([h[o], h[o + 1]]);
        let skills = std::array::from_fn(|i| (h[0x0A + 2 * i], h[0x0B + 2 * i]));
        let land_polygons = h[0x22..0xE2]
            .as_chunks::<24>().0.iter()
            .map(|set| {
                set.as_chunks::<3>().0.iter()
                    .map(|v| LandVertex { z: v[0], x: v[1], options: v[2] })
                    .take_while(|v| v.z != 0)
                    .collect::<Vec<_>>()
            })
            .filter(|p| !p.is_empty())
            .collect();
        let cameras = std::array::from_fn(|i| {
            let o = 0x15D + 8 * i;
            CameraPreset {
                y: u16_at(o) as i16,
                z: u16_at(o + 2) as i16,
                x: u16_at(o + 4) as i16,
                rotation: h[o + 6],
            }
        });
        let blocks = data[HEADER_LEN..HEADER_LEN + CELLS * 2]
            .as_chunks::<2>().0.iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let objects = data[HEADER_LEN + CELLS * 2..]
            .as_chunks::<2>().0.iter()
            .map(|b| ObjectCell { kind: b[0], extra: b[1] })
            .collect();
        Ok(Level {
            header: h.to_vec(),
            time_seconds: h[0],
            time_minutes: h[1],
            skills,
            land_polygons,
            release_rate: u16_at(0xE2) as i16,
            lemmings: u16_at(0xE4),
            save_requirement: u16_at(0xE6),
            texture_set: h[0xE8],
            land_gfx: h[0xE9],
            title: cstr(&h[0xF0..0x110]),
            object_set: h[0x110],
            sign_set: h[0x111],
            sea_gfx: h[0x112],
            anim_object: h[0x113],
            trap_type: h[0x114],
            sky_gfx: h[0x115],
            walls_set: h[0x117],
            water_speed_z: h[0x118] as i8,
            water_speed_x: h[0x119] as i8,
            flags: u16_at(0x11B),
            comment: cstr(&h[0x11D..0x13D]),
            face_limit: u16_at(0x13F),
            theme: h[0x143],
            music: h[0x151],
            border_kill: [h[0x153], h[0x154], h[0x155], h[0x156]],
            flags2: h[0x157],
            preview_pivot: [h[0x159], h[0x15A], h[0x15B]],
            ceiling_kill: h[0x15C],
            cameras,
            blocks,
            objects,
        })
    }

    pub fn block(&self, x: usize, y: usize, z: usize) -> BlockCell {
        BlockCell::from_raw(self.blocks[cell_index(x, y, z)])
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, cell: BlockCell) {
        self.blocks[cell_index(x, y, z)] = cell.to_raw();
    }

    pub fn block_raw(&self, x: usize, y: usize, z: usize) -> u16 {
        self.blocks[cell_index(x, y, z)]
    }

    pub fn object(&self, x: usize, y: usize, z: usize) -> ObjectCell {
        self.objects[cell_index(x, y, z)]
    }

    /// All cells as `(x, y, z, block, object)`, in X, Y, Z order.
    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, usize, BlockCell, ObjectCell)> + '_ {
        (0..SIZE_X).flat_map(move |x| {
            (0..SIZE_Y).flat_map(move |y| {
                (0..SIZE_Z).map(move |z| (x, y, z, self.block(x, y, z), self.object(x, y, z)))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_order() {
        assert_eq!(cell_index(0, 0, 1), 1);
        assert_eq!(cell_index(0, 1, 0), 32);
        assert_eq!(cell_index(1, 0, 0), 512);
        assert_eq!(cell_index(31, 15, 31), CELLS - 1);
    }

    #[test]
    fn block_bits() {
        let c = BlockCell::from_raw(0b000101_0011_10_1100);
        assert_eq!(c, BlockCell { id: 5, shape: 3, rotation: 2, segments: 0b1100 });
    }
}
