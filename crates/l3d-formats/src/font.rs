//! Bitmap fonts: fixed-size glyphs stored one after another, one per
//! character code from a first code upwards. See `docs/spec/ui-graphics.md`.

use crate::Error;
use crate::image::{IndexedImage, cells};

/// A fixed-cell bitmap font. Palette index 0 is the background.
#[derive(Debug, Clone)]
pub struct Font {
    pub glyph_width: usize,
    pub glyph_height: usize,
    /// Character code of the first glyph.
    pub first: u8,
    /// Glyphs for codes `first`, `first + 1`, ….
    pub glyphs: Vec<IndexedImage>,
}

impl Font {
    /// Reads a font whose glyphs fill `data` exactly.
    pub fn parse(
        data: &[u8],
        glyph_width: usize,
        glyph_height: usize,
        first: u8,
    ) -> Result<Self, Error> {
        let len = glyph_width * glyph_height;
        if data.is_empty() || !data.len().is_multiple_of(len) {
            return Err(Error::Format(format!(
                "{} bytes is not a whole number of {glyph_width}×{glyph_height} glyphs",
                data.len()
            )));
        }
        let glyphs = cells(data, glyph_width, glyph_height, data.len() / len)?;
        if first as usize + glyphs.len() > 256 {
            return Err(Error::Format("font runs past character code 255".into()));
        }
        Ok(Font {
            glyph_width,
            glyph_height,
            first,
            glyphs,
        })
    }

    /// The glyph for character code `c`, if the font has one.
    pub fn glyph(&self, c: u8) -> Option<&IndexedImage> {
        self.glyphs.get(c.checked_sub(self.first)? as usize)
    }
}

/// `GFX/TITLE.FNT`: 166 glyphs of 10×14 for codes 33 (`!`) to 198. Codes
/// 33–122 are an italic ASCII font; codes 123–198 are pieces of small logos.
pub const TITLE_FONT: (usize, usize, u8) = (10, 14, b'!');

/// Reads `GFX/TITLE.FNT`.
pub fn title_font(data: &[u8]) -> Result<Font, Error> {
    let (w, h, first) = TITLE_FONT;
    Font::parse(data, w, h, first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_lookup_by_code() {
        let data: Vec<u8> = (0..12).collect();
        let f = Font::parse(&data, 2, 2, b'A').unwrap();
        assert_eq!(f.glyphs.len(), 3);
        assert_eq!(f.glyph(b'B').unwrap().pixels, vec![4, 5, 6, 7]);
        assert!(f.glyph(b'@').is_none());
        assert!(f.glyph(b'D').is_none());
        assert!(Font::parse(&data[1..], 2, 2, b'A').is_err());
        assert!(Font::parse(&data, 2, 2, 254).is_err());
    }
}
