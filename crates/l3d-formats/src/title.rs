//! Title-screen and main-menu graphics: the spinning logo (`GFX/TITLE.MHC`),
//! the menu buttons (`GFX/TITLE.RNC`) and the lemming lettering of the code
//! screen (`GFX/LEMMINGS.FNT`). See `docs/spec/ui-graphics.md`.

use crate::Error;
use crate::image::{IndexedImage, cells};

/// Width of a `TITLE.MHC` frame.
pub const LOGO_WIDTH: usize = 256;
/// Height of a `TITLE.MHC` frame.
pub const LOGO_HEIGHT: usize = 64;
/// Number of frames in `TITLE.MHC`.
pub const LOGO_FRAMES: usize = 49;

/// Decodes a file of zero-run-length-encoded cells, each `width` pixels wide.
///
/// Each cell is a 16-bit little-endian byte count (including those two
/// bytes) followed by pixel data. A non-zero byte is one pixel; a zero byte
/// is followed by a count `n`: `n` transparent (index 0) pixels, or, when
/// `n` is 0, transparent pixels up to the end of the current row. The cell's
/// height is its pixel count divided by `width`, which must divide exactly.
pub fn decode_rle_cells(data: &[u8], width: usize) -> Result<Vec<IndexedImage>, Error> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let size = data
            .get(pos..pos + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or_else(|| Error::Format(format!("RLE cell header at {pos} cut short")))?;
        let body = data
            .get(pos + 2..pos + size.max(2))
            .ok_or_else(|| Error::Format(format!("RLE cell at {pos} ({size} bytes) runs past the end")))?;
        if size < 2 {
            return Err(Error::Format(format!("RLE cell at {pos} has size {size}")));
        }
        out.push(decode_rle(body, width).map_err(|e| Error::Format(format!("RLE cell {}: {e}", out.len())))?);
        pos += size;
    }
    Ok(out)
}

/// Decodes one cell body (see [`decode_rle_cells`]).
fn decode_rle(body: &[u8], width: usize) -> Result<IndexedImage, String> {
    let mut px = Vec::with_capacity(body.len() * 2);
    let mut bytes = body.iter().copied();
    while let Some(b) = bytes.next() {
        if b != 0 {
            px.push(b);
            continue;
        }
        let n = bytes.next().ok_or("zero run without a count")? as usize;
        let x = px.len() % width;
        let n = if n == 0 { width - x } else { n };
        if x + n > width {
            return Err(format!("run of {n} at column {x} crosses the row end"));
        }
        px.resize(px.len() + n, 0);
    }
    if !px.len().is_multiple_of(width) {
        return Err(format!("{} pixels is not a whole number of {width}-pixel rows", px.len()));
    }
    let height = px.len() / width;
    Ok(IndexedImage { width, height, pixels: px })
}

/// Size of `GFX/TITLE.RNC` once unpacked.
pub const MENU_LEN: usize = 5 * 64 * 64 + 5 * 32 * 32 + 15 * 16 * 16;

/// The main-menu art in `GFX/TITLE.RNC`.
#[derive(Debug, Clone)]
pub struct MenuArt {
    /// Five 64×64 buttons, a lemming holding a sign: Play (F1), Code (F2),
    /// Options (F12), difficulty rating, Exit (Esc). Each has a hole (index 0)
    /// where the face goes.
    pub buttons: Vec<IndexedImage>,
    /// Five 32×32 rating signs: Practice, Fun, Tricky, Taxing, Mayhem.
    pub ratings: Vec<IndexedImage>,
    /// Fifteen 16×16 faces: five heads, each as eyes open, half closed and
    /// closed.
    pub faces: Vec<IndexedImage>,
}

impl MenuArt {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != MENU_LEN {
            return Err(Error::Format(format!("TITLE.RNC is {} bytes, expected {MENU_LEN}", data.len())));
        }
        let (b, rest) = data.split_at(5 * 64 * 64);
        let (r, f) = rest.split_at(5 * 32 * 32);
        Ok(MenuArt { buttons: cells(b, 64, 64, 5)?, ratings: cells(r, 32, 32, 5)?, faces: cells(f, 16, 16, 15)? })
    }
}

/// Side of a `LEMMINGS.FNT` frame.
pub const LETTER_SIZE: usize = 32;
/// Frames of the idle animation at the start of `LEMMINGS.FNT`.
pub const LETTER_IDLE_FRAMES: usize = 6;
/// Frames per letter in `LEMMINGS.FNT`.
pub const LETTER_FRAMES: usize = 20;
/// Total frames in `LEMMINGS.FNT`: idle, A–Z, and one closing frame.
pub const LETTER_TOTAL: usize = LETTER_IDLE_FRAMES + 26 * LETTER_FRAMES + 1;

/// `GFX/LEMMINGS.FNT`: 32×32 frames of a small crowd of lemmings that forms
/// each letter A–Z.
#[derive(Debug, Clone)]
pub struct LemmingLetters {
    /// All frames, in file order.
    pub frames: Vec<IndexedImage>,
}

impl LemmingLetters {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let len = LETTER_TOTAL * LETTER_SIZE * LETTER_SIZE;
        if data.len() != len {
            return Err(Error::Format(format!("LEMMINGS.FNT is {} bytes, expected {len}", data.len())));
        }
        Ok(LemmingLetters { frames: cells(data, LETTER_SIZE, LETTER_SIZE, LETTER_TOTAL)? })
    }

    /// The idle frames: the lemmings standing in a row, shuffling.
    pub fn idle(&self) -> &[IndexedImage] {
        &self.frames[..LETTER_IDLE_FRAMES]
    }

    /// The 20 frames of letter `c` (`A`–`Z`, either case): from standing in a
    /// row, through forming the letter, back to the row.
    pub fn letter(&self, c: u8) -> Option<&[IndexedImage]> {
        let i = c.to_ascii_uppercase().checked_sub(b'A').filter(|&i| i < 26)? as usize;
        let start = LETTER_IDLE_FRAMES + i * LETTER_FRAMES;
        Some(&self.frames[start..start + LETTER_FRAMES])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_runs_and_row_ends() {
        // Width 4: "00 00" fills a whole row; "00 02" is two transparent
        // pixels; "00 00" mid-row fills to the row end.
        let body = [0, 0, 0, 2, 5, 6, 7, 0, 0];
        let mut file = vec![(body.len() + 2) as u8, 0];
        file.extend_from_slice(&body);
        file.extend_from_slice(&[4, 0, 9, 9]); // second cell: one partial row
        let err = decode_rle_cells(&file, 4).unwrap_err();
        assert!(err.to_string().contains("cell 1"));
        file.truncate(file.len() - 4);
        file.extend_from_slice(&[5, 0, 9, 0, 0]);
        let c = decode_rle_cells(&file, 4).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].width, c[0].height), (4, 3));
        assert_eq!(c[0].pixels, vec![0, 0, 0, 0, 0, 0, 5, 6, 7, 0, 0, 0]);
        assert_eq!(c[1].pixels, vec![9, 0, 0, 0]);
    }

    #[test]
    fn rle_rejects_runs_across_rows() {
        let file = [4, 0, 0, 5];
        assert!(decode_rle_cells(&file, 4).is_err());
        assert!(decode_rle_cells(&[9, 0, 1], 4).is_err());
    }

    #[test]
    fn letter_ranges() {
        let data = vec![0u8; LETTER_TOTAL * 1024];
        let l = LemmingLetters::parse(&data).unwrap();
        assert_eq!(l.idle().len(), 6);
        assert_eq!(l.letter(b'a').unwrap().len(), 20);
        assert!(std::ptr::eq(&l.letter(b'Z').unwrap()[19], &l.frames[525]));
        assert!(l.letter(b'1').is_none());
        assert!(MenuArt::parse(&vec![0u8; MENU_LEN]).is_ok());
    }
}
