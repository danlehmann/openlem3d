//! Vectors and matrices (glam's), and rectangles.

pub use glam::*;

/// An axis-aligned rectangle from `min` to `max`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self::from_corners(Vec2::new(x0, y0), Vec2::new(x1, y1))
    }

    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Rect {
            min: a.min(b),
            max: a.max(b),
        }
    }

    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half = size.abs() / 2.0;
        Rect {
            min: center - half,
            max: center + half,
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }

    pub fn is_empty(&self) -> bool {
        self.min.cmpge(self.max).any()
    }

    /// Whether `p` is inside or on the edge.
    pub fn contains(&self, p: Vec2) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }

    /// The overlap with `other` (empty if none).
    pub fn intersect(&self, other: Rect) -> Rect {
        let mut r = Rect {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        };
        r.min = r.min.min(r.max);
        r
    }
}

/// An axis-aligned rectangle of whole pixels, from `min` to `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct URect {
    pub min: UVec2,
    pub max: UVec2,
}

impl URect {
    pub fn from_corners(a: UVec2, b: UVec2) -> Self {
        URect {
            min: a.min(b),
            max: a.max(b),
        }
    }

    pub fn size(&self) -> UVec2 {
        self.max - self.min
    }
}
