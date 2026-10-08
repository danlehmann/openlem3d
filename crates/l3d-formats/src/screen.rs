//! Full-screen pictures: the intro slides (`GFX/INTRO*`), the theme scenes
//! (`GFX/SCENE*`), the loading banner (`GFX/LOADING.RNC`) and the Windows
//! bitmaps in `BMPS/`. See `docs/spec/ui-graphics.md`.

use crate::Error;
use crate::image::{IndexedImage, Palette, VGA_PALETTE_LEN, cells, vga_palette};

/// A picture with its own palette.
#[derive(Debug, Clone)]
pub struct Picture {
    pub image: IndexedImage,
    pub palette: Palette,
}

/// Low-resolution picture size (`.RNC` slides and scenes).
pub const LOW_RES: (usize, usize) = (320, 200);
/// High-resolution picture size (`.SVG` slides and scenes).
pub const HIGH_RES: (usize, usize) = (640, 480);

/// Reads an intro slide: a VGA palette followed by `width × height` pixels.
/// `GFX/INTROn.RNC` is 320×200 and `GFX/INTROn.SVG` 640×480.
pub fn intro(data: &[u8], (width, height): (usize, usize)) -> Result<Picture, Error> {
    if data.len() != VGA_PALETTE_LEN + width * height {
        return Err(Error::Format(format!(
            "intro slide is {} bytes, expected palette + {width}×{height}",
            data.len()
        )));
    }
    let (pal, px) = data.split_at(VGA_PALETTE_LEN);
    Ok(Picture {
        image: IndexedImage::new(width, height, px.to_vec())?,
        palette: vga_palette(pal)?,
    })
}

/// Width of the `GFX/LOADING.RNC` banner.
pub const LOADING_WIDTH: usize = 320;
/// Height of the `GFX/LOADING.RNC` banner.
pub const LOADING_HEIGHT: usize = 60;

/// Reads `GFX/LOADING.RNC`: a 320×60 "Now Loading" banner followed by its
/// VGA palette.
pub fn loading(data: &[u8]) -> Result<Picture, Error> {
    let n = LOADING_WIDTH * LOADING_HEIGHT;
    if data.len() != n + VGA_PALETTE_LEN {
        return Err(Error::Format(format!(
            "LOADING.RNC is {} bytes, expected {}",
            data.len(),
            n + VGA_PALETTE_LEN
        )));
    }
    Ok(Picture {
        image: IndexedImage::new(LOADING_WIDTH, LOADING_HEIGHT, data[..n].to_vec())?,
        palette: vga_palette(&data[n..])?,
    })
}

/// A theme scene: `GFX/SCENEnnn.RNC` (320×200, palette in `SCENEnnn.PAL`)
/// or `GFX/SCENEnnn.SVG` (640×480, palette in `SCENEnnn.SVP`).
#[derive(Debug, Clone)]
pub struct Scene {
    pub image: IndexedImage,
    /// Four square prompt cells drawn in the scene's palette: left mouse
    /// button, right mouse button, and the left and right halves of an
    /// "ENTER" key. 16×16 in `.RNC` files and 32×32 in `.SVG` files; absent
    /// from scene 0.
    pub prompts: Vec<IndexedImage>,
}

/// Reads a scene's pixels. The file holds `width × height` pixels, optionally
/// followed by four square prompt cells as wide as `width / 20`.
pub fn scene(data: &[u8], (width, height): (usize, usize)) -> Result<Scene, Error> {
    let n = width * height;
    let side = width / 20;
    let prompts = match data.len().checked_sub(n) {
        Some(0) => Vec::new(),
        Some(extra) if extra == 4 * side * side => cells(&data[n..], side, side, 4)?,
        _ => {
            return Err(Error::Format(format!(
                "scene is {} bytes, expected {n} or {}",
                data.len(),
                n + 4 * side * side
            )));
        }
    };
    Ok(Scene {
        image: IndexedImage::new(width, height, data[..n].to_vec())?,
        prompts,
    })
}

/// Reads an uncompressed 8-bit Windows BMP (`BITMAPINFOHEADER`), as found in
/// `BMPS/800BMPS` (800×600) and `BMPS/1024BMPS` (1024×768). Rows are
/// returned top to bottom.
pub fn bmp(data: &[u8]) -> Result<Picture, Error> {
    let bad = |m: &str| Error::Format(format!("BMP: {m}"));
    let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if data.get(..2) != Some(b"BM") {
        return Err(bad("missing BM signature"));
    }
    let offset = u32_at(10).ok_or_else(|| bad("short header"))? as usize;
    let header = u32_at(14).ok_or_else(|| bad("short header"))? as usize;
    let width = u32_at(18).ok_or_else(|| bad("short header"))? as i32;
    let height = u32_at(22).ok_or_else(|| bad("short header"))? as i32;
    let bits = u16_at(28).ok_or_else(|| bad("short header"))?;
    let compression = u32_at(30).ok_or_else(|| bad("short header"))?;
    let used = u32_at(46).ok_or_else(|| bad("short header"))? as usize;
    if header < 40 || bits != 8 || compression != 0 || width <= 0 || height == 0 {
        return Err(bad("only uncompressed 8-bit bitmaps are supported"));
    }
    let colours = if used == 0 { 256 } else { used.min(256) };
    let pal_at = 14 + header;
    let pal_raw = data
        .get(pal_at..pal_at + colours * 4)
        .ok_or_else(|| bad("palette cut short"))?;
    let mut palette = [[0u8; 3]; 256];
    for (p, c) in palette.iter_mut().zip(pal_raw.as_chunks::<4>().0) {
        *p = [c[2], c[1], c[0]];
    }
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let stride = w.div_ceil(4) * 4;
    let rows = data
        .get(offset..offset + stride * h)
        .ok_or_else(|| bad("pixel data cut short"))?;
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let src = if height > 0 { h - 1 - y } else { y };
        pixels.extend_from_slice(&rows[src * stride..src * stride + w]);
    }
    Ok(Picture {
        image: IndexedImage::new(w, h, pixels)?,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intro_palette_comes_first() {
        let mut data = vec![0u8; VGA_PALETTE_LEN + 4];
        data[0] = 63;
        data[VGA_PALETTE_LEN..].copy_from_slice(&[1, 2, 3, 4]);
        let p = intro(&data, (2, 2)).unwrap();
        assert_eq!(p.palette[0], [255, 0, 0]);
        assert_eq!(p.image.get(1, 1), 4);
        assert!(intro(&data[1..], (2, 2)).is_err());
    }

    #[test]
    fn scene_with_and_without_prompts() {
        // width 40 → 2×2 prompt cells
        let plain = vec![0u8; 40 * 2];
        assert!(scene(&plain, (40, 2)).unwrap().prompts.is_empty());
        let mut with = plain.clone();
        with.extend(0..16u8);
        let s = scene(&with, (40, 2)).unwrap();
        assert_eq!(s.prompts.len(), 4);
        assert_eq!(s.prompts[3].pixels, vec![12, 13, 14, 15]);
        assert!(scene(&with[1..], (40, 2)).is_err());
    }

    #[test]
    fn bmp_bottom_up_with_padding() {
        // 3×2 image: stride 4; bottom row stored first.
        let mut d = Vec::new();
        d.extend_from_slice(b"BM");
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&(14u32 + 40 + 1024).to_le_bytes());
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&3i32.to_le_bytes());
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&8u16.to_le_bytes());
        d.extend_from_slice(&[0; 24]);
        let mut pal = vec![0u8; 1024];
        pal[4..8].copy_from_slice(&[10, 20, 30, 0]);
        d.extend_from_slice(&pal);
        d.extend_from_slice(&[4, 5, 6, 0, 1, 2, 3, 0]);
        let p = bmp(&d).unwrap();
        assert_eq!(p.image.pixels, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(p.palette[1], [30, 20, 10]);
    }

    #[test]
    fn loading_palette_comes_last() {
        let mut data = vec![0u8; LOADING_WIDTH * LOADING_HEIGHT + VGA_PALETTE_LEN];
        data[LOADING_WIDTH * LOADING_HEIGHT + 3] = 63;
        assert_eq!(loading(&data).unwrap().palette[1], [255, 0, 0]);
    }
}
