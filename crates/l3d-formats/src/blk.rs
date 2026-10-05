//! `LEVELS/BLK.nnn`: the dictionary of 64 block definitions referenced by a
//! level's block grid. See `docs/spec/blk.md`.

use crate::Error;

pub const BLOCKS: usize = 64;
pub const DEF_LEN: usize = 22;
pub const FILE_LEN: usize = BLOCKS * DEF_LEN;

/// The six faces of a block, in storage order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceDir {
    PosZ = 0,
    NegZ = 1,
    PosX = 2,
    NegX = 3,
    PosY = 4,
    NegY = 5,
}

impl FaceDir {
    pub const ALL: [FaceDir; 6] =
        [FaceDir::PosZ, FaceDir::NegZ, FaceDir::PosX, FaceDir::NegX, FaceDir::PosY, FaceDir::NegY];
}

/// Block behaviour flags (byte 2 of a definition).
pub mod flags {
    pub const DOUBLE_SIDED: u8 = 0x01;
    pub const STEEL: u8 = 0x02;
    pub const LIQUID: u8 = 0x04;
    pub const UNKNOWN_08: u8 = 0x08;
    pub const NON_SOLID_LEMMINGS: u8 = 0x10;
    pub const NO_SPLAT: u8 = 0x20;
    pub const SLIPPERY: u8 = 0x40;
    pub const NON_SOLID_CAMERA: u8 = 0x80;
}

/// Face modifier bits (byte 1 of a face).
pub mod modifiers {
    pub const REVERSE_SIDE: u8 = 0x08;
    pub const ANIMATE_5: u8 = 0x10;
    pub const ANIMATE_4: u8 = 0x20;
    pub const ANIMATE_SPECIAL: u8 = 0x40;
    pub const COLOR0_TRANSPARENT: u8 = 0x80;
}

/// Appearance of one face of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    /// Tile index into the level's `TEXTURE` file (0xFF in unused definitions).
    pub texture: u8,
    /// Bit set of [`modifiers`].
    pub modifiers: u8,
    /// Darkening level, 0 (none) to 8 (darkest).
    pub shading: u8,
}

/// One block definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDef {
    pub unknown0: u8,
    pub unknown1: u8,
    /// Bit set of [`flags`].
    pub flags: u8,
    pub unknown3: u8,
    /// Faces in [`FaceDir`] order.
    pub faces: [Face; 6],
}

impl BlockDef {
    pub fn face(&self, dir: FaceDir) -> Face {
        self.faces[dir as usize]
    }

    /// True for the placeholder pattern found in unused slots.
    pub fn is_placeholder(&self) -> bool {
        self.faces.iter().all(|f| f.texture == 0xFF && f.modifiers == 0xFF)
    }
}

/// A block dictionary.
#[derive(Debug, Clone)]
pub struct BlockSet {
    pub defs: [BlockDef; BLOCKS],
}

impl BlockSet {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != FILE_LEN {
            return Err(Error::Format(format!("BLK is {} bytes, expected {FILE_LEN}", data.len())));
        }
        let defs = std::array::from_fn(|i| {
            let d = &data[i * DEF_LEN..(i + 1) * DEF_LEN];
            BlockDef {
                unknown0: d[0],
                unknown1: d[1],
                flags: d[2],
                unknown3: d[3],
                faces: std::array::from_fn(|f| {
                    let o = 4 + f * 3;
                    Face { texture: d[o], modifiers: d[o + 1], shading: d[o + 2] }
                }),
            }
        });
        Ok(BlockSet { defs })
    }
}
