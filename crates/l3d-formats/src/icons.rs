//! `GFX/ICONS.RNC`: in-game panel icons and the two small fonts, stored
//! back to back without a header. Each part has its own cell size. See
//! `docs/spec/ui-graphics.md`.

use crate::Error;
use crate::font::Font;
use crate::image::{IndexedImage, cells};

/// Size of `GFX/ICONS.RNC` once unpacked.
pub const ICONS_LEN: usize = 63_961;

/// Byte ranges of the parts, in file order.
pub mod offsets {
    /// 57 icons of 16×16.
    pub const SMALL: usize = 0;
    /// 46 icons of 24×24.
    pub const PANEL: usize = 14_592;
    /// One 32×32 umbrella, then IN, OUT and clock labels of 32×16.
    pub const LABELS: usize = 41_088;
    /// Wooden panel pieces, only partly worked out: see [`super::HEIGHT_BAR`]
    /// and [`super::KNOB`].
    pub const UNKNOWN: usize = 43_648;
    /// 7×8 font, codes 33–122.
    pub const SMALL_FONT: usize = 48_121;
    /// 10×12 font, codes 33–122.
    pub const LARGE_FONT: usize = 53_161;
}

/// The height bar at the right edge of the screen, 12×200: a wooden column
/// with a groove down the middle and a red corner mark, at this offset into
/// the wooden pieces.
pub const HEIGHT_BAR: usize = 948;
/// The height bar's knob, 9×8: a yellow square in a black frame, at this
/// offset into the wooden pieces.
pub const KNOB: usize = 3_345;

/// Indices into [`Icons::panel`]. Where an icon comes in a green and a red
/// version, the green one is listed; the red one follows it.
pub mod panel {
    /// Paw prints walking: 14 frames (pause).
    pub const PAUSE: std::ops::Range<usize> = 0..14;
    pub const PLUS: usize = 14;
    pub const FAST_FORWARD: usize = 16;
    /// A small red play triangle.
    pub const PLAY: usize = 17;
    pub const TURN_CLOCKWISE: usize = 18;
    pub const MINUS: usize = 20;
    /// A lemming's face, eyes open; 23 is the same face squinting.
    pub const FACE: usize = 22;
    pub const TURN_ANTICLOCKWISE: usize = 24;
    /// A black bomb (nuke).
    pub const BOMB: usize = 26;
    /// A mushroom-cloud explosion growing and fading: 11 frames.
    pub const EXPLOSION: std::ops::Range<usize> = 27..38;
    /// A red arrow pointing down.
    pub const RED_DOWN: usize = 38;
    /// A green arrow pointing down, animated: 6 frames.
    pub const GREEN_DOWN: std::ops::Range<usize> = 39..45;
    /// A cine camera.
    pub const CAMERA: usize = 45;
}

/// The decoded parts of `GFX/ICONS.RNC`. All parts use `GFX/LM3D.PAL` with
/// index 0 transparent.
#[derive(Debug, Clone)]
pub struct Icons {
    /// 57 icons of 16×16: block shapes, editor buttons, small action
    /// pictures, arrows and mouse buttons.
    pub small: Vec<IndexedImage>,
    /// 46 icons of 24×24 for the in-game panel; see [`panel`].
    pub panel: Vec<IndexedImage>,
    /// A 32×32 red-and-white umbrella.
    pub umbrella: IndexedImage,
    /// 32×16 labels: "IN" and "OUT" spelt in lemming lettering, and a clock.
    pub labels: Vec<IndexedImage>,
    /// The height bar and its knob (see [`HEIGHT_BAR`] and [`KNOB`]).
    pub height_bar: IndexedImage,
    pub knob: IndexedImage,
    /// Bytes between the labels and the small font, not decoded yet.
    pub unknown: Vec<u8>,
    /// 7×8 font, codes 33 (`!`) to 122 (`z`). Codes 35–37 hold a slider knob
    /// instead of `#$%`.
    pub small_font: Font,
    /// 10×12 font, codes 33 (`!`) to 122 (`z`).
    pub large_font: Font,
}

impl Icons {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        use offsets::*;
        if data.len() != ICONS_LEN {
            return Err(Error::Format(format!(
                "ICONS.RNC is {} bytes, expected {ICONS_LEN}",
                data.len()
            )));
        }
        let mut labels = cells(&data[LABELS..UNKNOWN], 32, 16, 5)?;
        let umbrella = IndexedImage::new(
            32,
            32,
            [labels.remove(0).pixels, labels.remove(0).pixels].concat(),
        )?;
        Ok(Icons {
            small: cells(&data[SMALL..PANEL], 16, 16, 57)?,
            panel: cells(&data[PANEL..LABELS], 24, 24, 46)?,
            umbrella,
            labels,
            height_bar: IndexedImage::new(
                12,
                200,
                data[UNKNOWN + HEIGHT_BAR..][..12 * 200].to_vec(),
            )?,
            knob: IndexedImage::new(9, 8, data[UNKNOWN + KNOB..][..9 * 8].to_vec())?,
            unknown: data[UNKNOWN..SMALL_FONT].to_vec(),
            small_font: Font::parse(&data[SMALL_FONT..LARGE_FONT], 7, 8, b'!')?,
            large_font: Font::parse(&data[LARGE_FONT..], 10, 12, b'!')?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_add_up() {
        use offsets::*;
        assert_eq!(PANEL - SMALL, 57 * 16 * 16);
        assert_eq!(LABELS - PANEL, 46 * 24 * 24);
        assert_eq!(UNKNOWN - LABELS, 32 * 32 + 3 * 32 * 16);
        assert_eq!(LARGE_FONT - SMALL_FONT, 90 * 7 * 8);
        assert_eq!(ICONS_LEN - LARGE_FONT, 90 * 10 * 12);
        let mut data = vec![0u8; ICONS_LEN];
        data[LABELS + 32 * 16] = 7; // first pixel of the umbrella's lower half
        let icons = Icons::parse(&data).unwrap();
        assert_eq!(icons.umbrella.get(0, 16), 7);
        assert_eq!(icons.labels.len(), 3);
        assert_eq!(icons.small_font.glyph(b'z').unwrap().width, 7);
        assert_eq!(icons.large_font.glyphs.len(), 90);
    }
}
