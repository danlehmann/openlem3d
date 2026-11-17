//! Routes for camera glides that keep clear of blocks (ours, enhanced mode):
//! a glide between two views goes straight when nothing is in the way, and
//! otherwise round what is, along a path found over a half-unit grid of the
//! places the camera may be, shortened where it can see further along and
//! rounded at its corners. Where the destination is closed off (a lemming
//! in a sealed cell), the path crosses the least wall it can, and the glide
//! cuts across that wall from one face to the other.

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
/// How far from the glide's ends (in grid points along each axis) a free
/// grid point to start or finish the search at is looked for.
const END_REACH: i32 = 3;
/// How many times a step through a wall costs a step through open space:
/// enough that any way round, however long, is taken first, and a wall is
/// crossed only where the destination is closed off.
const WALL_COST: u64 = 1000;
/// The most grid points a closed-off space has (a sealed room of some
/// 2,500 cubic units).
const SMALL_SPACE: usize = 20_000;

/// The way a glide takes: the points it passes through, `points[0]` its
/// start and the last its end, and per leg between two points (`cut[i]`
/// for `points[i]` to `points[i + 1]`) whether the leg crosses a wall,
/// which the view does at once rather than glide through it.
#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub points: Vec<Vec3>,
    pub cut: Vec<bool>,
}

impl Route {
    fn straight(from: Vec3, to: Vec3) -> Self {
        Self {
            points: vec![from, to],
            cut: vec![false],
        }
    }

    /// The length glided along, leaving out the legs cut across walls.
    pub(crate) fn length(&self) -> f32 {
        self.legs()
            .filter(|(_, _, cut)| !cut)
            .map(|(a, b, _)| a.distance(b))
            .sum()
    }

    /// The point `d` units glided along the way (the legs cut across walls
    /// passed at once).
    pub(crate) fn along(&self, d: f32) -> Vec3 {
        let mut left = d.max(0.0);
        for (a, b, cut) in self.legs() {
            if cut {
                continue;
            }
            let leg = a.distance(b);
            if left <= leg && leg > 0.0 {
                return a.lerp(b, left / leg);
            }
            left -= leg;
        }
        *self.points.last().unwrap()
    }

    /// The legs: their ends and whether each is cut across a wall.
    pub(crate) fn legs(&self) -> impl Iterator<Item = (Vec3, Vec3, bool)> + '_ {
        self.points
            .windows(2)
            .zip(&self.cut)
            .map(|(w, &c)| (w[0], w[1], c))
    }
}

/// Whether the camera can move in a straight line from `a` to `b` without
/// passing through a block (checked in steps of at most [`CAMERA_STEP`]).
/// An end that touches a block (a lemming's eyes close to the floor or a
/// wall) may: the line is blocked then only while still leaving the start
/// or already reaching the end, never between two clear stretches. A line
/// blocked all along is clear only when short (both ends in one tight
/// spot).
pub(crate) fn clear_line(level: &Level, blocks: &BlockSet, a: Vec3, b: Vec3) -> bool {
    let steps = ((b - a).length() / CAMERA_STEP).ceil().max(1.0) as usize;
    let blocked: Vec<bool> = (0..=steps)
        .map(|i| camera_blocked(level, blocks, a.lerp(b, i as f32 / steps as f32)))
        .collect();
    let Some(first) = blocked.iter().position(|b| !b) else {
        return a.distance(b) <= 2.0 * CAMERA_RADIUS;
    };
    let last = blocked.iter().rposition(|b| !b).unwrap();
    !blocked[first..=last].contains(&true)
}

type Cell = [i32; 3];

/// The position of grid point `g`.
fn point(g: Cell) -> Vec3 {
    Vec3::new(
        (g[0] as f32 + 0.5) * SPACING - MARGIN,
        (g[1] as f32 + 0.5) * SPACING,
        (g[2] as f32 + 0.5) * SPACING - MARGIN,
    )
}

fn index(g: Cell) -> usize {
    (g[0] as usize * NY + g[1] as usize) * NZ + g[2] as usize
}

/// What is at a grid point.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Spot {
    /// The camera may be there.
    Free,
    /// Too close to a block, or inside one.
    Wall,
    /// Off the grid or below the ground plane: never passed.
    Never,
}

/// The grid's spots, each found out when first asked about.
struct Grid<'a> {
    level: &'a Level,
    blocks: &'a BlockSet,
    known: std::cell::RefCell<Vec<Option<Spot>>>,
}

impl<'a> Grid<'a> {
    fn new(level: &'a Level, blocks: &'a BlockSet) -> Self {
        Self {
            level,
            blocks,
            known: std::cell::RefCell::new(vec![None; NX * NY * NZ]),
        }
    }

    fn at(&self, g: Cell) -> Spot {
        let on_grid = (0..NX as i32).contains(&g[0])
            && (0..NY as i32).contains(&g[1])
            && (0..NZ as i32).contains(&g[2]);
        if !on_grid {
            return Spot::Never;
        }
        *self.known.borrow_mut()[index(g)].get_or_insert_with(|| {
            let p = point(g);
            if p.y < crate::scene_build::GROUND_Y + CAMERA_RADIUS {
                Spot::Never
            } else if camera_blocked(self.level, self.blocks, p) {
                Spot::Wall
            } else {
                Spot::Free
            }
        })
    }
}

/// The free grid point nearest `p` in plain view of it, if one is within
/// [`END_REACH`].
fn nearest_free(grid: &Grid, p: Vec3) -> Option<Cell> {
    let c = ((p + Vec3::new(MARGIN, 0.0, MARGIN)) / SPACING - 0.5).round();
    let c = [c.x as i32, c.y as i32, c.z as i32];
    let mut best: Option<(Cell, f32)> = None;
    for di in -END_REACH..=END_REACH {
        for dj in -END_REACH..=END_REACH {
            for dk in -END_REACH..=END_REACH {
                let g = [c[0] + di, c[1] + dj, c[2] + dk];
                if grid.at(g) != Spot::Free {
                    continue;
                }
                let d = point(g).distance(p);
                if best.is_none_or(|(_, b)| d < b)
                    && clear_line(grid.level, grid.blocks, p, point(g))
                {
                    best = Some((g, d));
                }
            }
        }
    }
    best.map(|(g, _)| g)
}

/// The way for the camera from `from` to `to`: straight when the straight
/// line is clear; otherwise round the blocks in the way, crossing the least wall it can
/// (cut) only where the destination is closed off. `None` when either end
/// has no open space near it.
pub(crate) fn route(level: &Level, blocks: &BlockSet, from: Vec3, to: Vec3) -> Option<Route> {
    if clear_line(level, blocks, from, to) {
        return Some(Route::straight(from, to));
    }
    let grid = Grid::new(level, blocks);
    let start = nearest_free(&grid, from)?;
    let goal = nearest_free(&grid, to)?;
    // Searched from the end in a small closed-off space, if one is: a
    // search from outside it would cover all the open space before trying
    // a wall.
    let cells = if !closed_off(&grid, start) && closed_off(&grid, goal) {
        let mut c = search(&grid, goal, start)?;
        c.reverse();
        c
    } else {
        search(&grid, start, goal)?
    };
    // Split into stretches through open space, each from a free point to a
    // free point, with the walls between them cut across.
    let mut stretches: Vec<Vec<Vec3>> = vec![vec![from]];
    let mut in_wall = false;
    for &g in &cells {
        if grid.at(g) == Spot::Free {
            if in_wall {
                stretches.push(Vec::new());
                in_wall = false;
            }
            stretches.last_mut().unwrap().push(point(g));
        } else {
            in_wall = true;
        }
    }
    stretches.last_mut().unwrap().push(to);
    let mut out = Route {
        points: Vec::new(),
        cut: Vec::new(),
    };
    for s in stretches {
        let s = round(level, blocks, shorten(level, blocks, &s));
        if !out.points.is_empty() {
            out.cut.push(true);
        }
        for (i, p) in s.into_iter().enumerate() {
            if i > 0 {
                out.cut.push(false);
            }
            out.points.push(p);
        }
    }
    Some(out)
}

/// Whether the open space grid point `g` is in is small: fewer than
/// [`SMALL_SPACE`] free points, reached from each other along the axes.
fn closed_off(grid: &Grid, g: Cell) -> bool {
    let mut seen = vec![false; NX * NY * NZ];
    seen[index(g)] = true;
    let mut count = 1;
    let mut todo = vec![g];
    while let Some(c) = todo.pop() {
        for a in 0..3 {
            for s in [-1, 1] {
                let mut n = c;
                n[a] += s;
                if grid.at(n) == Spot::Free && !std::mem::replace(&mut seen[index(n)], true) {
                    count += 1;
                    if count >= SMALL_SPACE {
                        return false;
                    }
                    todo.push(n);
                }
            }
        }
    }
    true
}

/// A* from grid point `start` to `goal` over the 26 neighbours of each
/// point: free points (diagonally only where the points between are free
/// too, so as not to clip a block's edge), and walls at [`WALL_COST`] times
/// the cost.
fn search(grid: &Grid, start: Cell, goal: Cell) -> Option<Vec<Cell>> {
    let dist = |a: Cell, b: Cell| {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f32).sqrt()
    };
    // Costs in thousandths of a grid step, for the heap's ordering.
    let milli = |d: f32| (d * 1000.0) as u64;
    let mut cost = vec![u64::MAX; NX * NY * NZ];
    let mut came = vec![usize::MAX; NX * NY * NZ];
    let mut open = BinaryHeap::new();
    cost[index(start)] = 0;
    open.push(Reverse((milli(dist(start, goal)), index(start), start)));
    while let Some(Reverse((f, at, g))) = open.pop() {
        if f > cost[at].saturating_add(milli(dist(g, goal))) {
            continue;
        }
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
        let here = grid.at(g);
        for di in -1..=1 {
            for dj in -1..=1 {
                for dk in -1..=1 {
                    if di == 0 && dj == 0 && dk == 0 {
                        continue;
                    }
                    let n = [g[0] + di, g[1] + dj, g[2] + dk];
                    let there = grid.at(n);
                    let step = match there {
                        Spot::Never => continue,
                        Spot::Wall => milli(dist(g, n)) * WALL_COST,
                        Spot::Free => milli(dist(g, n)),
                    };
                    let c = cost[at] + step;
                    if c >= cost[index(n)] {
                        continue;
                    }
                    if here == Spot::Free
                        && there == Spot::Free
                        && !between([di, dj, dk])
                            .all(|o| grid.at([g[0] + o[0], g[1] + o[1], g[2] + o[2]]) == Spot::Free)
                    {
                        continue;
                    }
                    cost[index(n)] = c;
                    came[index(n)] = at;
                    open.push(Reverse((c + milli(dist(n, goal)), index(n), n)));
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

/// The steps from a grid point to the points between it and its neighbour
/// `d` steps away: along any non-empty choice of the axes `d` moves along
/// (the neighbour itself included).
fn between(d: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let moved: u8 = (0..3).filter(|&a| d[a] != 0).map(|a| 1 << a).sum();
    (1..8u8)
        .filter(move |s| s & !moved == 0)
        .map(move |s| [0, 1, 2].map(|a| if s & (1 << a) != 0 { d[a] } else { 0 }))
}
