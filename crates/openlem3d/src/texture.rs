//! Pixel art as pictures (sRGB RGBA8; the UI samples them nearest, so the
//! pixels stay sharp when scaled).

use l3d_engine::image::Image;
use l3d_formats::gamedata::Palette;
use l3d_formats::image::IndexedImage;

/// A `width`×`height` picture of the RGBA bytes `rgba`.
pub(crate) fn rgba_image(width: u32, height: u32, rgba: Vec<u8>) -> Image {
    Image::new(width, height, rgba)
}

/// `img` in `pal`'s colours; with `transparent0`, colour 0 shows through.
pub(crate) fn indexed_image(img: &IndexedImage, pal: &Palette, transparent0: bool) -> Image {
    rgba_image(
        img.width as u32,
        img.height as u32,
        img.to_rgba(pal, transparent0),
    )
}
