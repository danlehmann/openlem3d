//! Colours: given in sRGB or linear RGB, with alpha.

/// A colour in sRGB.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Srgba {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

/// A colour in linear RGB, as the GPU blends.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinearRgba {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl LinearRgba {
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        LinearRgba {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn to_f32_array(self) -> [f32; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Srgba(Srgba),
    LinearRgba(LinearRgba),
}

impl Default for Color {
    fn default() -> Self {
        Color::WHITE
    }
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl Color {
    pub const WHITE: Color = Color::srgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::srgb(0.0, 0.0, 0.0);
    pub const NONE: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

    pub const fn srgb(red: f32, green: f32, blue: f32) -> Color {
        Color::srgba(red, green, blue, 1.0)
    }

    pub const fn srgba(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Color::Srgba(Srgba {
            red,
            green,
            blue,
            alpha,
        })
    }

    pub fn alpha(&self) -> f32 {
        match self {
            Color::Srgba(c) => c.alpha,
            Color::LinearRgba(c) => c.alpha,
        }
    }

    pub fn with_alpha(self, alpha: f32) -> Color {
        match self {
            Color::Srgba(c) => Color::Srgba(Srgba { alpha, ..c }),
            Color::LinearRgba(c) => Color::LinearRgba(LinearRgba { alpha, ..c }),
        }
    }

    pub fn to_linear(&self) -> LinearRgba {
        match *self {
            Color::Srgba(c) => LinearRgba {
                red: to_linear(c.red),
                green: to_linear(c.green),
                blue: to_linear(c.blue),
                alpha: c.alpha,
            },
            Color::LinearRgba(c) => c,
        }
    }
}
