//! Signals set off by lemmings on a circuit board (ours, Enhanced and
//! Ultra): a lemming standing on one of the board's traces sends a pulse of
//! light from its feet along the traces joined to it, round their corners.
//!
//! The board's picture repeats over the level, so where a pulse is cannot
//! live in the picture: a field over the level's grid, at the picture's
//! resolution, holds for each texel when the latest pulse's front reaches
//! it, which the shader turns into light as the front passes
//! (`scene.wgsl`).

use std::collections::VecDeque;
use std::sync::Arc;

use l3d_formats::level::{SIZE_X, SIZE_Z};

/// A circuit board's picture, as the field needs it.
#[derive(Clone)]
pub struct CircuitBoard {
    /// Which texels of the picture are traces, row by row.
    pub trace: Arc<[bool]>,
    /// The picture's width and height in texels.
    pub texels: usize,
    /// Texels per grid unit as the picture is laid over the level.
    pub per_unit: f32,
}

/// How far a pulse runs, in texels; how fast, in texels a second (as the
/// board's own signals, `SIGNAL_SPEED`); and how often one lemming may send
/// one, in seconds.
const REACH: u32 = 120;
const SPEED: f32 = 45.0;
const EVERY: f32 = 1.2;
/// How far round a pulse's start the trace glows longer, in texels, how
/// long, and the step that is counted in (milliseconds).
const LINGER_REACH: u32 = 4;
const LINGER: u32 = 500 / LINGER_STEP;
const LINGER_STEP: u32 = 4;

/// The field of a circuit board's pulses.
pub struct SignalField {
    board: CircuitBoard,
    /// Its width and height in texels, over the level's grid.
    pub size: (u32, u32),
    /// When the latest pulse's front reaches each texel, in milliseconds of
    /// scene time in the low 24 bits (0: never), and in the high 8, how long
    /// it stays lit there in [`LINGER_STEP`]s (0: as a front passing),
    /// row by row.
    pub arrivals: Vec<u32>,
    /// The texels changed since the renderer last took them: x and y from,
    /// and to (exclusive).
    pub dirty: Option<[u32; 4]>,
    /// When each lemming last sent a pulse, in seconds.
    sent: Vec<f32>,
    /// Which field this is, so that the renderer uploads a new one whole.
    pub generation: u64,
}

impl SignalField {
    pub fn new(board: CircuitBoard, generation: u64) -> Self {
        let size = (
            (SIZE_X as f32 * board.per_unit).ceil() as u32,
            (SIZE_Z as f32 * board.per_unit).ceil() as u32,
        );
        SignalField {
            arrivals: vec![0; (size.0 * size.1) as usize],
            size,
            board,
            dirty: None,
            sent: Vec::new(),
            generation,
        }
    }

    /// Sends a pulse from the feet of each lemming in `feet` (world
    /// positions, by lemming; `None` for one not standing on the board)
    /// that stands on a trace and has not sent one lately, `now` seconds in.
    pub fn step(&mut self, feet: &[Option<[f32; 3]>], now: f32) {
        self.sent.resize(feet.len(), f32::NEG_INFINITY);
        for (i, foot) in feet.iter().enumerate() {
            let Some([x, _, z]) = *foot else { continue };
            if now - self.sent[i] < EVERY {
                continue;
            }
            let (fx, fz) = (x * self.board.per_unit, z * self.board.per_unit);
            if fx < 0.0 || fz < 0.0 {
                continue;
            }
            let (fx, fz) = (fx as u32, fz as u32);
            if fx >= self.size.0 || fz >= self.size.1 || !self.is_trace(fx, fz) {
                continue;
            }
            self.sent[i] = now;
            self.send(fx, fz, now);
        }
    }

    fn is_trace(&self, x: u32, z: u32) -> bool {
        let n = self.board.texels;
        self.board.trace[(z as usize % n) * n + x as usize % n]
    }

    /// A pulse from texel `(x, z)`, `now` seconds in: its front reaches each
    /// joined trace texel within [`REACH`] after a time by its distance
    /// (diagonal steps counting as one, so that the front lies straight
    /// across a trace). A texel still lit by an earlier pulse keeps it.
    fn send(&mut self, x: u32, z: u32, now: f32) {
        let (w, h) = self.size;
        let now_ms = (now * 1000.0) as u32 & 0xFF_FFFF;
        let tail_ms = (1000.0 * TAIL / SPEED) as u32;
        let mut dist = std::collections::HashMap::new();
        dist.insert((x, z), 0u32);
        let mut queue = VecDeque::from([(x, z)]);
        let mut rect = [x, z, x + 1, z + 1];
        while let Some((cx, cz)) = queue.pop_front() {
            let d = dist[&(cx, cz)];
            let arrival = (now_ms + (d as f32 * 1000.0 / SPEED) as u32) & 0xFF_FFFF;
            // Around the feet the trace glows a while as the pulse sets off.
            let linger = if d <= LINGER_REACH { LINGER } else { 0 };
            let at = (cz * w + cx) as usize;
            let old = self.arrivals[at] & 0xFF_FFFF;
            if old == 0 || old + tail_ms < now_ms || arrival < old {
                self.arrivals[at] = arrival.max(1) | linger << 24;
                rect = [
                    rect[0].min(cx),
                    rect[1].min(cz),
                    rect[2].max(cx + 1),
                    rect[3].max(cz + 1),
                ];
            }
            if d == REACH {
                continue;
            }
            for (dx, dz) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
                (-1, -1),
            ] {
                let (nx, nz) = (cx as i32 + dx, cz as i32 + dz);
                if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                    continue;
                }
                let next = (nx as u32, nz as u32);
                if !dist.contains_key(&next) && self.is_trace(next.0, next.1) {
                    dist.insert(next, d + 1);
                    queue.push_back(next);
                }
            }
        }
        self.dirty = Some(match self.dirty {
            Some(r) => [
                r[0].min(rect[0]),
                r[1].min(rect[1]),
                r[2].max(rect[2]),
                r[3].max(rect[3]),
            ],
            None => rect,
        });
    }
}

/// How long a pulse's fading tail is, in texels (as the board's own
/// signals, `SIGNAL_TAIL`).
pub const TAIL: f32 = 14.0;

impl SignalField {
    /// Texels per grid unit.
    pub fn per_unit(&self) -> f32 {
        self.board.per_unit
    }
}

impl SignalField {
    /// Whether this field is for `board` (the same picture).
    pub fn is_for(&self, board: &CircuitBoard) -> bool {
        Arc::ptr_eq(&self.board.trace, &board.trace)
    }
}
