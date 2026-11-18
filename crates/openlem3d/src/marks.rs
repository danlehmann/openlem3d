//! Marks the lemmings leave on an icy level (ours, Ultra mode): footprints
//! where they walk, shown on snow, and a mark where they slide: a groove in
//! snow, a scratch on ice.
//!
//! A field over the level's grid holds, per texel, how deep a footprint is,
//! how strong a slide mark, and the height of the surface marked, so that a
//! mark shows only on that surface (`scene.wgsl`). Marks fade over a minute
//! or so.

use l3d_formats::level::{SIZE_X, SIZE_Z};

/// Texels per grid unit.
pub const PER_UNIT: f32 = 40.0;
/// Steps per unit the surface's height is counted in (the field's blue).
pub const HEIGHT_STEPS: f32 = 8.0;
/// How far a lemming walks between prints, in units; how far apart its
/// feet are; a print's length and width.
const STRIDE: f32 = 0.14;
const FEET_APART: f32 = 0.11;
const PRINT_LENGTH: f32 = 0.12;
const PRINT_WIDTH: f32 = 0.08;
/// How far apart a slide's scratches are stamped along it, in units, and
/// how wide one is.
const SLIDE_STEP: f32 = 0.02;
const SLIDE_WIDTH: f32 = 0.2;
/// How often the marks fade, in seconds, and by how much each time.
const FADE_EVERY: f32 = 1.0;
const FADE: f32 = 0.96;
/// How far round a mark, in texels, its surface's height is kept: past the
/// ridge of snow it pushes up.
const RIDGE_TEXELS: f32 = 7.0;

/// What a lemming is doing where it is, for its marks.
#[derive(Clone, Copy)]
pub enum Step {
    /// Walking with its feet here.
    Walk([f32; 3]),
    /// Sliding with its feet here.
    Slide([f32; 3]),
}

/// Where a lemming's marks have got to.
#[derive(Clone, Copy)]
struct Track {
    /// Where its feet were last followed to.
    at: [f32; 3],
    /// Where it left its last footprint.
    printed: [f32; 3],
    /// Whether its left foot prints next.
    left: bool,
}

/// The marks over a level.
pub struct MarkField {
    /// Its width and height in texels, over the level's grid.
    pub size: (u32, u32),
    /// Four bytes a texel, row by row: footprint depth, slide mark, the
    /// surface's height in [`HEIGHT_STEPS`]ths of a unit, unused.
    pub texels: Vec<u8>,
    /// The texels changed since the renderer last took them: x and y from,
    /// and to (exclusive).
    pub dirty: Option<[u32; 4]>,
    /// The texels holding any mark, as [`Self::dirty`].
    marked: Option<[u32; 4]>,
    /// Per lemming, where its marks have got to.
    last: Vec<Option<Track>>,
    /// When the marks last faded, in seconds.
    faded: f32,
    /// Which field this is, so that the renderer uploads a new one whole.
    pub generation: u64,
}

impl MarkField {
    pub fn new(generation: u64) -> Self {
        let size = (
            (SIZE_X as f32 * PER_UNIT) as u32,
            (SIZE_Z as f32 * PER_UNIT) as u32,
        );
        MarkField {
            texels: vec![0; (size.0 * size.1 * 4) as usize],
            size,
            dirty: None,
            marked: None,
            last: Vec::new(),
            faded: 0.0,
            generation,
        }
    }

    /// The lemmings this field has followed.
    pub fn followed(&self) -> usize {
        self.last.len()
    }

    /// Leaves marks for each lemming's step (by lemming; `None` for one
    /// leaving none) and fades the old ones, `now` seconds in.
    pub fn step(&mut self, steps: &[Option<Step>], now: f32) {
        self.last.resize(steps.len(), None);
        for (i, step) in steps.iter().enumerate() {
            let Some(step) = *step else {
                self.last[i] = None;
                continue;
            };
            let p = match step {
                Step::Walk(p) | Step::Slide(p) => p,
            };
            let Some(mut track) = self.last[i] else {
                self.last[i] = Some(Track {
                    at: p,
                    printed: p,
                    left: false,
                });
                continue;
            };
            // Far from where it was (put back by a rewind): no mark across.
            if distance(track.at, p) > 1.0 {
                self.last[i] = Some(Track {
                    at: p,
                    printed: p,
                    left: track.left,
                });
                continue;
            }
            // A footprint each stride, walking.
            let d = distance(track.printed, p);
            if matches!(step, Step::Slide(_)) {
                track.printed = p;
            } else if d >= STRIDE {
                let along = [(p[0] - track.printed[0]) / d, (p[2] - track.printed[2]) / d];
                // Across the way it goes, to one side or the other.
                let side = if track.left { 0.5 } else { -0.5 } * FEET_APART;
                let foot = [p[0] - along[1] * side, p[1], p[2] + along[0] * side];
                self.stamp(foot, along, PRINT_LENGTH, PRINT_WIDTH, 0);
                track.printed = p;
                track.left = !track.left;
            }
            // A mark along a slide.
            let d = distance(track.at, p);
            if matches!(step, Step::Slide(_)) && d >= SLIDE_STEP {
                let from = track.at;
                let along = [(p[0] - from[0]) / d, (p[2] - from[2]) / d];
                let n = (d / SLIDE_STEP).ceil() as u32;
                for k in 1..=n {
                    let f = k as f32 / n as f32;
                    let at = std::array::from_fn(|j| from[j] + (p[j] - from[j]) * f);
                    self.stamp(at, along, SLIDE_STEP * 2.0, SLIDE_WIDTH, 1);
                }
            }
            if d >= SLIDE_STEP {
                track.at = p;
            }
            self.last[i] = Some(track);
        }
        if now - self.faded >= FADE_EVERY || now < self.faded {
            self.faded = now;
            self.fade();
        }
    }

    /// Marks an ellipse at `at` (world), its length along `along` (a unit
    /// vector in x and z), `length` by `width` units, in channel `channel`
    /// (0: footprint, 1: slide), deepest in the middle.
    fn stamp(&mut self, at: [f32; 3], along: [f32; 2], length: f32, width: f32, channel: usize) {
        let (w, h) = self.size;
        let (cx, cz) = (at[0] * PER_UNIT, at[2] * PER_UNIT);
        // The surface's height reaches past the mark, over the ridge of
        // snow round it (`scene.wgsl`'s `SNOW_RIDGE_OUT`).
        let reach = length.max(width) * PER_UNIT + RIDGE_TEXELS;
        if cx + reach < 0.0 || cz + reach < 0.0 {
            return;
        }
        let x0 = (cx - reach).floor().max(0.0) as u32;
        let z0 = (cz - reach).floor().max(0.0) as u32;
        let x1 = ((cx + reach).ceil() as u32 + 1).min(w);
        let z1 = ((cz + reach).ceil() as u32 + 1).min(h);
        if x0 >= x1 || z0 >= z1 {
            return;
        }
        let height = (at[1] * HEIGHT_STEPS).round().clamp(0.0, 255.0) as u8;
        for z in z0..z1 {
            for x in x0..x1 {
                let dx = (x as f32 + 0.5 - cx) / PER_UNIT;
                let dz = (z as f32 + 0.5 - cz) / PER_UNIT;
                let a = (dx * along[0] + dz * along[1]) / (length / 2.0);
                let b = (-dx * along[1] + dz * along[0]) / (width / 2.0);
                let r = a * a + b * b;
                let i = ((z * w + x) * 4) as usize;
                if r >= 1.0 {
                    if self.texels[i] | self.texels[i + 1] == 0 {
                        self.texels[i + 2] = height;
                    }
                    continue;
                }
                let depth = (255.0 * (1.0 - r * r)) as u8;
                self.texels[i + channel] = self.texels[i + channel].max(depth);
                self.texels[i + 2] = height;
            }
        }
        let rect = [x0, z0, x1, z1];
        self.dirty = Some(union(self.dirty, rect));
        self.marked = Some(union(self.marked, rect));
    }

    /// Every mark a little fainter.
    fn fade(&mut self) {
        let Some([x0, z0, x1, z1]) = self.marked else {
            return;
        };
        let w = self.size.0;
        let mut any = false;
        for z in z0..z1 {
            for x in x0..x1 {
                let i = ((z * w + x) * 4) as usize;
                let t = &mut self.texels[i..i + 2];
                if t[0] | t[1] != 0 {
                    t[0] = (t[0] as f32 * FADE) as u8;
                    t[1] = (t[1] as f32 * FADE) as u8;
                    any = true;
                }
            }
        }
        if any {
            self.dirty = Some(union(self.dirty, [x0, z0, x1, z1]));
        } else {
            self.marked = None;
        }
    }
}

/// The smallest rectangle holding `a` (if any) and `b`.
fn union(a: Option<[u32; 4]>, b: [u32; 4]) -> [u32; 4] {
    match a {
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
        None => b,
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
