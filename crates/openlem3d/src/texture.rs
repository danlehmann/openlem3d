//! Pixel art as Bevy images: 2D, sRGB RGBA8, sampled nearest so the
//! pixels stay sharp when scaled.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use l3d_formats::gamedata::Palette;
use l3d_formats::image::IndexedImage;

/// A `width`×`height` image of the RGBA bytes `rgba`, kept in `usages`.
pub(crate) fn rgba_image(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    usages: RenderAssetUsages,
) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        usages,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// `img` in `pal`'s colours, on the GPU only; with `transparent0`, colour 0
/// shows through.
pub(crate) fn indexed_image(img: &IndexedImage, pal: &Palette, transparent0: bool) -> Image {
    rgba_image(
        img.width as u32,
        img.height as u32,
        img.to_rgba(pal, transparent0),
        RenderAssetUsages::RENDER_WORLD,
    )
}
