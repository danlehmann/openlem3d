//! Pictures: RGBA, 8 bits a channel, sRGB, rows top to bottom.

use glam::UVec2;

use crate::asset::Asset;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes.
    pub data: Vec<u8>,
}

impl Asset for Image {}

impl Image {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(data.len(), (width * height * 4) as usize, "RGBA size");
        Image {
            width,
            height,
            data,
        }
    }

    pub fn size(&self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }
}
