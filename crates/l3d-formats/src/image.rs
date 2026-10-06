//! 8-bit indexed images and 6-bit VGA palettes: the building blocks of the
//! user-interface graphics. See `docs/spec/ui-graphics.md`.

use crate::Error;

/// A 256-colour palette with components scaled to 8 bits.
pub type Palette = [[u8; 3]; 256];

/// Size in bytes of a stored VGA palette: 256 × (R, G, B).
pub const VGA_PALETTE_LEN: usize = 768;

/// Converts a stored VGA palette (256 × R, G, B with 6-bit components,
/// 0–63) to 8 bits per component.
pub fn vga_palette(raw: &[u8]) -> Result<Palette, Error> {
    if raw.len() != VGA_PALETTE_LEN {
        return Err(Error::Format(format!("VGA palette is {} bytes, expected {VGA_PALETTE_LEN}", raw.len())));
    }
    Ok(std::array::from_fn(|i| {
        std::array::from_fn(|c| {
            let v = raw[i * 3 + c] & 0x3F;
            (v << 2) | (v >> 4)
        })
    }))
}

/// A rectangular image of palette indices, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedImage {
    pub width: usize,
    pub height: usize,
    /// `width × height` palette indices, row-major.
    pub pixels: Vec<u8>,
}

impl IndexedImage {
    /// Wraps `pixels`, which must hold exactly `width × height` indices.
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Result<Self, Error> {
        if pixels.len() != width * height {
            return Err(Error::Format(format!("{} pixels for a {width}×{height} image", pixels.len())));
        }
        Ok(IndexedImage { width, height, pixels })
    }

    /// The palette index at (`x`, `y`).
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// The `w × h` rectangle with its top-left corner at (`x`, `y`), which
    /// must lie inside the image.
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> IndexedImage {
        assert!(x + w <= self.width && y + h <= self.height, "crop outside the image");
        let pixels = (y..y + h).flat_map(|r| &self.pixels[r * self.width + x..r * self.width + x + w]).copied().collect();
        IndexedImage { width: w, height: h, pixels }
    }

    /// The image as RGBA8, with palette index 0 fully transparent when
    /// `index0_transparent` is set and every other pixel opaque.
    pub fn to_rgba(&self, palette: &Palette, index0_transparent: bool) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|&p| {
                let [r, g, b] = palette[p as usize];
                [r, g, b, if p == 0 && index0_transparent { 0 } else { 255 }]
            })
            .collect()
    }
}

/// Cuts `count` images of `width × height` that are stored one after another
/// (each one row-major) from the start of `data`.
pub fn cells(data: &[u8], width: usize, height: usize, count: usize) -> Result<Vec<IndexedImage>, Error> {
    let len = width * height;
    if data.len() < len * count {
        return Err(Error::Format(format!("{} bytes hold fewer than {count} cells of {width}×{height}", data.len())));
    }
    Ok(data.chunks_exact(len).take(count).map(|c| IndexedImage { width, height, pixels: c.to_vec() }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_scales_six_bit_components() {
        let mut raw = vec![0u8; VGA_PALETTE_LEN];
        raw[3..6].copy_from_slice(&[63, 32, 1]);
        let pal = vga_palette(&raw).unwrap();
        assert_eq!(pal[0], [0, 0, 0]);
        assert_eq!(pal[1], [255, 130, 4]);
        assert!(vga_palette(&raw[1..]).is_err());
    }

    #[test]
    fn cells_and_crop() {
        let data: Vec<u8> = (0..12).collect();
        let c = cells(&data, 2, 3, 2).unwrap();
        assert_eq!(c[1].pixels, vec![6, 7, 8, 9, 10, 11]);
        assert_eq!(c[1].get(1, 2), 11);
        assert!(cells(&data, 2, 3, 3).is_err());
        let img = IndexedImage::new(3, 4, data).unwrap();
        assert_eq!(img.crop(1, 2, 2, 2).pixels, vec![7, 8, 10, 11]);
    }
}
