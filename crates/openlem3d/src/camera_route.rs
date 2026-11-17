//! Routes for camera glides that keep clear of blocks (ours, enhanced mode):
//! a glide between two views goes straight when nothing is in the way, and
//! otherwise round what is, along a path found over a half-unit grid of the
//! places the camera may be, shortened where it can see further along and
//! rounded at its corners.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use l3d_engine::prelude::*;
use l3d_formats::blk::BlockSet;
use l3d_formats::level::{Level, SIZE_X, SIZE_Z};

use crate::{CAMERA_RADIUS, CAMERA_STEP, camera_blocked};

/// The grid's spacing, in units.
const SPACING: f32 = 0.5;
/// How far the grid reaches beyond the level's blocks sideways (preset
/// cameras may stand outside them), and how high it reaches, in units.
const MARGIN: f32 = 8.0;
const HEIGHT: f32 = 20.0;
/// Grid points along each axis.
const NX: usize = (SIZE_X as f32 + 2.0 * MARGIN) as usize * 2;
const NY: usize = HEIGHT as usize * 2;
const NZ: usize = (SIZE_Z as f32 + 2.0 * MARGIN) as usize * 2;
/// The most grid points a search looks at before giving up.
const SEARCH_LIMIT: usize = 200_000;
/// How far from the glide's ends (in grid points along each axis) a free
/// grid point to start or finish the search at is looked for.
const END_REACH: i32 = 3;

/// Whether the camera can move in a straight line from `a` to `b` without
/// touching a block (checked in steps of at most [`CAMERA_STEP`]).
pub(crate) fn clear_line(level: &Level, blocks: &BlockSet, a: Vec3, b: Vec3) -> bool {
    let steps = ((b - a).length() / CAMERA_STEP).ceil().max(1.0) as usize;
    (0..=steps).all(|i| !camera_blocked(level, blocks, a.lerp(b, i as f32 / steps as f32)))
}

/// The position of grid point `(i, j, k)`.
fn point(i: usize, j: usize, k: usize) -> Vec3 {
    Vec3::new(
        (i as f32 + 0.5) * SPACING - MARGIN,
        (j as f32 + 0.5) * SPACING,
        (k as f32 + 0.5) * SPACING - MARGIN,
    )
}

fn index(i: usize, j: usize, k: usize) -> usize {
    (i * NY + j) * NZ + k
}

/// The grid points the camera may be at: clear of blocks and above the
/// ground plane, each found out when first asked about.
struct Free<'a> {
    level: &'a Level,
    blocks: &'a BlockSet,
    /// Per grid point: 0 not yet known, 1 free, 2 not.
    known: std::cell::RefCell<Vec<u8>>,
}

impl<'a> Free<'a> {
    fn new(level: &'a Level, blocks: &'a BlockSet) -> Self {
        Self {
            level,
            blocks,
            known: std::cell::RefCell::new(vec![0; NX * NY * NZ]),
        }
    }

    fn at(&self, i: i32, j: i32, k: i32) -> bool {
        if !((0..NX as i32).contains(&i)
            && (0..NY as i32).contains(&j)
            && (0..NZ as i32).contains(&k))
        {
            return false;
        }
        let (i, j, k) = (i as usize, j as usize, k as usize);
        let mut known = self.known.borrow_mut();
        let slot = &mut known[index(i, j, k)];
        if *slot == 0 {
            let p = point(i, j, k);
            let floor = crate::scene_build::GROUND_Y + CAMERA_RADIUS;
            *slot = if p.y >= floor && !camera_blocked(self.level, self.blocks, p) {
                1
            } else {
                2
            };
        }
        *slot == 1
    }
}

/// The grid point nearest `p` that is free and in plain view of it, if one
/// is within [`END_REACH`].
fn nearest_free(free: &Free, level: &Level, blocks: &BlockSet, p: Vec3) -> Option<[i32; 3]> {
    let c = ((p + Vec3::new(MARGIN, 0.0, MARGIN)) / SPACING - 0.5).round();
    let c = [c.x as i32, c.y as i32, c.z as i32];
    let mut best: Option<([i32; 3], f32)> = None;
    for di in -END_REACH..=END_REACH {
        for dj in -END_REACH..=END_REACH {
            for dk in -END_REACH..=END_REACH {
                let g = [c[0] + di, c[1] + dj, c[2] + dk];
                if !free.at(g[0], g[1], g[2]) {
                    continue;
                }
                let q = point(g[0] as usize, g[1] as usize, g[2] as usize);
                let d = q.distance(p);
                if best.is_none_or(|(_, b)| d < b) && clear_line(level, blocks, p, q) {
                    best = Some((g, d));
                }
            }
        }
    }
    best.map(|(g, _)| g)
}

/// A route for the camera from `from` to `to` that keeps clear of blocks:
/// the points to pass through, `from` first and `to` last (the two alone
/// when the straight line between them is clear); `None` when blocks are in
/// the way and no way round them is found.
pub(crate) fn route(level: &Level, blocks: &BlockSet, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    if clear_line(level, blocks, from, to) {
        return Some(vec![from, to]);
    }
    let free = Free::new(level, blocks);
    let start = nearest_free(&free, level, blocks, from)?;
    let goal = nearest_free(&free, level, blocks, to)?;
    let cells = search(&free, level, blocks, start, goal)?;
    let mut path = vec![from];
    path.extend(
        cells
            .iter()
            .map(|g| point(g[0] as usize, g[1] as usize, g[2] as usize)),
    );
    path.push(to);
    let path = shorten(level, blocks, &path);
    Some(round(level, blocks, path))
}

/// A* from grid point `start` to `goal` over free points, each step to one
/// of the 26 neighbours along a clear line.
fn search(
    free: &Free,
    level: &Level,
    blocks: &BlockSet,
    start: [i32; 3],
    goal: [i32; 3],
) -> Option<Vec<[i32; 3]>> {
    let key = |g: [i32; 3]| index(g[0] as usize, g[1] as usize, g[2] as usize);
    let dist = |a: [i32; 3], b: [i32; 3]| {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f32).sqrt()
    };
    // Costs in thousandths of a grid step, for the heap's ordering.
    let milli = |d: f32| (d * 1000.0) as u32;
    let mut cost = vec![u32::MAX; NX * NY * NZ];
    let mut came = vec![usize::MAX; NX * NY * NZ];
    let mut open = BinaryHeap::new();
    cost[key(start)] = 0;
    open.push(Reverse((milli(dist(start, goal)), key(start), start)));
    let mut seen = 0;
    while let Some(Reverse((_, at, g))) = open.pop() {
        if g == goal {
            let mut path = vec![g];
            let mut i = at;
            while came[i] != usize::MAX {
                i = came[i];
                let (gi, rest) = (i / (NY * NZ), i % (NY * NZ));
                path.push([gi as i32, (rest / NZ) as i32, (rest % NZ) as i32]);
            }
            path.reverse();
            return Some(path);
        }
        seen += 1;
        if seen > SEARCH_LIMIT {
            return None;
        }
        let here = point(g[0] as usize, g[1] as usize, g[2] as usize);
        for di in -1..=1 {
            for dj in -1..=1 {
                for dk in -1..=1 {
                    if di == 0 && dj == 0 && dk == 0 {
                        continue;
                    }
                    let n = [g[0] + di, g[1] + dj, g[2] + dk];
                    if !free.at(n[0], n[1], n[2]) {
                        continue;
                    }
                    let c = cost[at] + milli(dist(g, n));
                    if c >= cost[key(n)] {
                        continue;
                    }
                    let there = point(n[0] as usize, n[1] as usize, n[2] as usize);
                    if (di != 0) as i32 + (dj != 0) as i32 + (dk != 0) as i32 > 1
                        && !clear_line(level, blocks, here, there)
                    {
                        continue;
                    }
                    cost[key(n)] = c;
                    came[key(n)] = at;
                    open.push(Reverse((c + milli(dist(n, goal)), key(n), n)));
                }
            }
        }
    }
    None
}

/// `path` with the points it can do without: from each kept point, on to
/// the furthest later one in plain view.
fn shorten(level: &Level, blocks: &BlockSet, path: &[Vec3]) -> Vec<Vec3> {
    let mut out = vec![path[0]];
    let mut i = 0;
    while i + 1 < path.len() {
        let mut j = path.len() - 1;
        while j > i + 1 && !clear_line(level, blocks, path[i], path[j]) {
            j -= 1;
        }
        out.push(path[j]);
        i = j;
    }
    out
}

/// `path` with its corners rounded (two rounds of cutting each corner a
/// quarter of the way along its legs), where the rounded path is clear.
fn round(level: &Level, blocks: &BlockSet, path: Vec<Vec3>) -> Vec<Vec3> {
    let mut p = path;
    for _ in 0..2 {
        if p.len() < 3 {
            break;
        }
        let mut q = vec![p[0]];
        for w in p.windows(2) {
            q.push(w[0].lerp(w[1], 0.25));
            q.push(w[0].lerp(w[1], 0.75));
        }
        // The ends stay where they are.
        q.remove(1);
        q.pop();
        q.push(*p.last().unwrap());
        if q.windows(2).all(|w| clear_line(level, blocks, w[0], w[1])) {
            p = q;
        } else {
            break;
        }
    }
    p
}

/// The length of the route through `points`.
pub(crate) fn length(points: &[Vec3]) -> f32 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The point `d` units along the route through `points`.
pub(crate) fn along(points: &[Vec3], d: f32) -> Vec3 {
    let mut left = d.max(0.0);
    for w in points.windows(2) {
        let leg = w[0].distance(w[1]);
        if left <= leg && leg > 0.0 {
            return w[0].lerp(w[1], left / leg);
        }
        left -= leg;
    }
    *points.last().unwrap()
}
