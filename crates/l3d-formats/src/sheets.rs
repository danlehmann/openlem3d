//! Headerless sprite sheets of equal-sized cells in the `GFX` folder. See
//! `docs/spec/ui-graphics.md`.

use crate::Error;
use crate::image::{IndexedImage, cells};

/// Layout of a sprite sheet: `count` cells of `cell_width × cell_height`,
/// stored as one image `columns` cells wide, rows top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sheet {
    /// Path on the CD.
    pub path: &'static str,
    pub cell_width: usize,
    pub cell_height: usize,
    /// Cells across one row of the stored image.
    pub columns: usize,
    pub count: usize,
    /// Path of the palette file, or `None` when the colours come from
    /// elsewhere (see the sheet's description).
    pub palette: Option<&'static str>,
}

impl Sheet {
    /// Exact size of the (unpacked) file.
    pub const fn file_len(&self) -> usize {
        self.cell_width * self.cell_height * self.count
    }

    /// Cuts the sheet's cells from the file contents.
    pub fn cut(&self, data: &[u8]) -> Result<Vec<IndexedImage>, Error> {
        if data.len() != self.file_len() {
            return Err(Error::Format(format!("{} is {} bytes, expected {}", self.path, data.len(), self.file_len())));
        }
        if self.columns == 1 {
            return cells(data, self.cell_width, self.cell_height, self.count);
        }
        let width = self.cell_width * self.columns;
        let rows = self.count.div_ceil(self.columns);
        let whole = IndexedImage::new(width, data.len() / width, data.to_vec())?;
        Ok((0..rows * self.columns)
            .take(self.count)
            .map(|i| {
                let (cx, cy) = (i % self.columns, i / self.columns);
                whole.crop(cx * self.cell_width, cy * self.cell_height, self.cell_width, self.cell_height)
            })
            .collect())
    }
}

const MAIN: Option<&str> = Some("GFX/LM3D.PAL");

/// Small animated lemmings in skill poses, 64 frames of 32×32.
pub const MINILEMM: Sheet =
    Sheet { path: "GFX/MINILEMM.GFX", cell_width: 32, cell_height: 32, columns: 1, count: 64, palette: MAIN };

/// Brush-stroke digits 1–5, a question mark, a down arrow and an empty
/// cell: 8 cells of 32×32 in two columns.
pub const BOMBNUMB: Sheet =
    Sheet { path: "GFX/BOMBNUMB.GFX", cell_width: 32, cell_height: 32, columns: 2, count: 8, palette: MAIN };

/// Three turning cogs, 3 frames of 88×99. The indices refer to the palette
/// stored in `GFX/LOADING.RNC`.
pub const COGS: Sheet =
    Sheet { path: "GFX/COGS.GFX", cell_width: 88, cell_height: 99, columns: 1, count: 3, palette: None };

/// Mouse pointers, 43 cells of 16×16.
pub const MOUSE: Sheet =
    Sheet { path: "GFX/MOUSE.RNC", cell_width: 16, cell_height: 16, columns: 1, count: 43, palette: MAIN };

/// A wedge-shaped block turning through 32 angles, 32×32.
pub const DEFLICON: Sheet =
    Sheet { path: "GFX/DEFLICON.RNC", cell_width: 32, cell_height: 32, columns: 1, count: 32, palette: MAIN };

/// 56 icons of 32×32 for level objects and their animations.
pub const PRACICON: Sheet =
    Sheet { path: "GFX/PRACICON.RNC", cell_width: 32, cell_height: 32, columns: 1, count: 56, palette: MAIN };

/// A lemming standing and waving, 8 frames of 22×32.
pub const ENDLEMMS: Sheet =
    Sheet { path: "GFX/ENDLEMMS.RNC", cell_width: 22, cell_height: 32, columns: 1, count: 8, palette: MAIN };

/// A lemming turning a wheel, 8 frames of 64×64.
pub const WINDER: Sheet =
    Sheet { path: "GFX/WINDER.RNC", cell_width: 64, cell_height: 64, columns: 1, count: 8, palette: MAIN };

/// Every sheet above.
pub const ALL: [Sheet; 8] = [MINILEMM, BOMBNUMB, COGS, MOUSE, DEFLICON, PRACICON, ENDLEMMS, WINDER];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_sheet_cells() {
        let s = Sheet { path: "x", cell_width: 2, cell_height: 1, columns: 2, count: 4, palette: None };
        let c = s.cut(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(c[1].pixels, vec![3, 4]);
        assert_eq!(c[2].pixels, vec![5, 6]);
        assert!(s.cut(&[0; 7]).is_err());
    }

    #[test]
    fn known_file_sizes() {
        let sizes = [65_536, 8_192, 26_136, 11_008, 32_768, 57_344, 5_632, 32_768];
        for (s, n) in ALL.iter().zip(sizes) {
            assert_eq!(s.file_len(), n, "{}", s.path);
        }
    }
}
