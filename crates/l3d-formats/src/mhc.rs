//! `LEMM/LEMM.MHC` and `LEMM/LEMM128.MHC`: run-length-trimmed lemming
//! sprite cells. See `docs/spec/lemmings.md`; format first described by
//! GuyPerfect [LF1590].

use crate::Error;

/// Number of entries in the cell manifest.
pub const MANIFEST_ENTRIES: usize = 568;
/// Size of the manifest in bytes.
pub const MANIFEST_LEN: usize = MANIFEST_ENTRIES * 4;
/// Distance between the data areas of consecutive animations.
const ANIMATION_STRIDE: usize = 0x4000;

/// One manifest entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellEntry {
    /// Animation the cell belongs to.
    pub animation: u8,
    pub flags: u8,
    /// Byte offset of the cell structure within the file.
    pub offset: usize,
}

/// A decoded square cell: `size × size` palette indices, 0 = transparent.
#[derive(Debug, Clone)]
pub struct Cell {
    pub size: usize,
    pub pixels: Vec<u8>,
}

/// A lemming sprite file.
pub struct MhcFile<'a> {
    data: &'a [u8],
    /// Cell width and height (64 for `LEMM.MHC`, 128 for `LEMM128.MHC`).
    pub size: usize,
    pub entries: Vec<CellEntry>,
}

impl<'a> MhcFile<'a> {
    pub fn parse(data: &'a [u8], size: usize) -> Result<Self, Error> {
        if data.len() < MANIFEST_LEN {
            return Err(Error::Format("MHC file shorter than its manifest".into()));
        }
        let entries = data[..MANIFEST_LEN]
            .chunks_exact(4)
            .map(|e| {
                let animation = e[0];
                let rel = u16::from_le_bytes([e[2], e[3]]) as usize;
                CellEntry { animation, flags: e[1], offset: MANIFEST_LEN + animation as usize * ANIMATION_STRIDE + rel }
            })
            .collect();
        Ok(MhcFile { data, size, entries })
    }

    /// Decodes the cell of manifest entry `i`.
    pub fn cell(&self, i: usize) -> Result<Cell, Error> {
        let e = self.entries.get(i).ok_or_else(|| Error::Format(format!("no MHC entry {i}")))?;
        let bad = || Error::Format(format!("MHC cell {i} out of bounds"));
        let n = self.size;
        let mut pixels = vec![0u8; n * n];
        for row in 0..n {
            let o = e.offset + row * 2;
            let rel = u16::from_le_bytes([*self.data.get(o).ok_or_else(bad)?, *self.data.get(o + 1).ok_or_else(bad)?]);
            let line = e.offset + rel as usize;
            let lead = *self.data.get(line).ok_or_else(bad)? as usize;
            let trail = *self.data.get(line + 1).ok_or_else(bad)? as usize;
            if lead + trail > n {
                return Err(Error::Format(format!("MHC cell {i} row {row}: lead {lead} + trail {trail} > {n}")));
            }
            let count = n - lead - trail;
            let src = self.data.get(line + 2..line + 2 + count).ok_or_else(bad)?;
            pixels[row * n + lead..row * n + lead + count].copy_from_slice(src);
        }
        Ok(Cell { size: n, pixels })
    }
}
