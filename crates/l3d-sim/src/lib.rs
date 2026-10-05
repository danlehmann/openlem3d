//! Deterministic Lemmings 3D simulation: lemmings moving through a level's
//! collision geometry at a fixed tick rate. No rendering or engine types.
//!
//! Behaviour constants are provisional until measured against the original
//! game; see `docs/spec/behaviour.md`.

pub mod world;

use l3d_formats::blk::{BlockSet, flags};
use l3d_formats::level::{Level, SIZE_X, SIZE_Y, SIZE_Z};
pub use world::{Floor, SUB, World};

/// Simulation ticks per second.
pub const TICKS_PER_SECOND: u32 = 30;

/// Walking speed, sub-units per tick (provisional).
const WALK_SPEED: i32 = SUB / 48;
/// Falling speed, sub-units per tick (provisional).
const FALL_SPEED: i32 = SUB / 12;
/// Highest step a walker climbs without turning (provisional: one segment).
const STEP_UP: i32 = SUB / 4;
/// Lowest drop a walker steps down without falling (provisional).
const STEP_DOWN: i32 = SUB / 4;
/// Falls longer than this splat (provisional).
const SPLAT_HEIGHT: i32 = 4 * SUB;
/// Height checked for headroom when walking (provisional).
const HEAD_HEIGHT: i32 = SUB / 2;
/// How far ahead of its centre a walker probes for walls.
const REACH: i32 = SUB / 4;
/// Duration of terminal animations, in ticks (provisional).
const EXIT_TICKS: u32 = 24;
const DEATH_TICKS: u32 = 30;

/// Block ids with hard-coded meaning (`docs/spec/blk.md`).
const ENTRANCE_ID: u8 = 0;
const EXIT_ID: u8 = 1;

/// One of the four horizontal headings lemmings walk in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    PosX,
    NegX,
    PosZ,
    NegZ,
}

impl Dir {
    /// Unit step in sub-unit coordinates.
    pub fn delta(self) -> [i32; 3] {
        match self {
            Dir::PosX => [1, 0, 0],
            Dir::NegX => [-1, 0, 0],
            Dir::PosZ => [0, 0, 1],
            Dir::NegZ => [0, 0, -1],
        }
    }

    pub fn reverse(self) -> Dir {
        match self {
            Dir::PosX => Dir::NegX,
            Dir::NegX => Dir::PosX,
            Dir::PosZ => Dir::NegZ,
            Dir::NegZ => Dir::PosZ,
        }
    }

    /// Quarter turn clockwise seen from above (+Z → −X → −Z → +X).
    pub fn clockwise(self) -> Dir {
        match self {
            Dir::PosZ => Dir::NegX,
            Dir::NegX => Dir::NegZ,
            Dir::NegZ => Dir::PosX,
            Dir::PosX => Dir::PosZ,
        }
    }

    /// Quarter turn anticlockwise seen from above; the block-rotation sense
    /// (+X → −Z → −X → +Z).
    pub fn anticlockwise(self) -> Dir {
        self.clockwise().reverse()
    }

    /// Heading as an angle for rendering: radians in the same convention as
    /// camera yaw (0 = −X, π/2 = −Z, π = +X, 3π/2 = +Z).
    pub fn yaw(self) -> f32 {
        use std::f32::consts::{FRAC_PI_2, PI};
        match self {
            Dir::NegX => 0.0,
            Dir::NegZ => FRAC_PI_2,
            Dir::PosX => PI,
            Dir::PosZ => 3.0 * FRAC_PI_2,
        }
    }
}

/// What a lemming is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Falling { from_y: i32 },
    Walking,
    Exiting,
    Splatting,
    Drowning,
    /// Killed by the level boundary (the original shows a lightning zap).
    Zapped,
}

#[derive(Debug, Clone)]
pub struct Lemming {
    /// Feet position, sub-units.
    pub pos: [i32; 3],
    pub dir: Dir,
    pub state: State,
    /// Ticks spent in the current state.
    pub state_ticks: u32,
    /// Set once the lemming has left play (saved or dead).
    pub gone: bool,
}

impl Lemming {
    fn set_state(&mut self, s: State) {
        self.state = s;
        self.state_ticks = 0;
    }
}

/// An entrance hatch: lemmings appear below it.
#[derive(Debug, Clone, Copy)]
pub struct Entrance {
    pub spawn: [i32; 3],
    pub dir: Dir,
}

/// Counters shown in the game's HUD.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub released: u32,
    pub saved: u32,
    pub dead: u32,
}

/// The state of a level being played.
pub struct Simulation {
    pub world: World,
    pub lemmings: Vec<Lemming>,
    pub entrances: Vec<Entrance>,
    /// Lemmings to release in total.
    pub to_release: u32,
    pub counts: Counts,
    /// Release rate, 1..=99 (higher releases faster).
    pub release_rate: i32,
    pub tick: u64,
    /// Ticks until the next release.
    release_timer: u32,
    next_entrance: usize,
    /// Kill boundary `[min_x, min_z, max_x, max_z]` in cells.
    border: [i32; 4],
    ceiling: i32,
}

impl Simulation {
    pub fn new(level: &Level, blocks: &BlockSet) -> Self {
        let world = World::new(level, blocks);
        let mut entrances = Vec::new();
        for (x, y, z, b, _) in level.cells() {
            // Entrances only work with the top two segments present and the
            // bottom two absent; at most four are active ([L3DEdit], unverified).
            if b.id == ENTRANCE_ID && b.segments == 0b1100 && entrances.len() < 4 {
                // Unrotated hatches release towards +Z ([L3DEdit]); each
                // rotation step turns the release direction +Z → +X → −Z → −X.
                // Inferred from LEVEL.000, whose hatch (rotation 1) sits on
                // the west edge of its island: only +X keeps lemmings on land.
                let mut dir = Dir::PosZ;
                for _ in 0..b.rotation {
                    dir = dir.anticlockwise();
                }
                let spawn = [x as i32 * SUB + SUB / 2, y as i32 * SUB + SUB / 2 - 1, z as i32 * SUB + SUB / 2];
                entrances.push(Entrance { spawn, dir });
            }
        }
        let b = level.border_kill;
        Simulation {
            world,
            lemmings: Vec::new(),
            entrances,
            to_release: level.lemmings as u32,
            counts: Counts::default(),
            release_rate: (level.release_rate as i32).clamp(1, 99),
            tick: 0,
            release_timer: 0,
            next_entrance: 0,
            border: [b[0] as i32, b[1] as i32, b[2] as i32, b[3] as i32],
            ceiling: level.ceiling_kill as i32,
        }
    }

    /// Ticks between releases (provisional: classic Lemmings timing scaled to
    /// our tick rate).
    fn release_interval(&self) -> u32 {
        let frames_17hz = (99 - self.release_rate) / 2 + 4;
        (frames_17hz as u32 * TICKS_PER_SECOND).div_ceil(17)
    }

    /// Advances the simulation by one tick.
    pub fn step(&mut self) {
        self.tick += 1;
        if self.counts.released < self.to_release && !self.entrances.is_empty() {
            if self.release_timer == 0 {
                let e = self.entrances[self.next_entrance % self.entrances.len()];
                self.next_entrance += 1;
                self.lemmings.push(Lemming {
                    pos: e.spawn,
                    dir: e.dir,
                    state: State::Falling { from_y: e.spawn[1] },
                    state_ticks: 0,
                    gone: false,
                });
                self.counts.released += 1;
                self.release_timer = self.release_interval();
            } else {
                self.release_timer -= 1;
            }
        }
        for i in 0..self.lemmings.len() {
            if !self.lemmings[i].gone {
                let mut l = self.lemmings[i].clone();
                self.update(&mut l);
                self.lemmings[i] = l;
            }
        }
    }

    /// Lemmings still in play.
    pub fn out(&self) -> usize {
        self.lemmings.iter().filter(|l| !l.gone).count()
    }

    fn in_bounds(&self, p: [i32; 3]) -> bool {
        let c = p.map(|v| v.div_euclid(SUB));
        let [min_x, min_z, max_x, max_z] = self.border;
        c[0] >= min_x && c[0] <= max_x && c[2] >= min_z && c[2] <= max_z && c[1] <= self.ceiling
    }

    fn update(&mut self, l: &mut Lemming) {
        l.state_ticks += 1;
        match l.state {
            State::Exiting => {
                if l.state_ticks >= EXIT_TICKS {
                    l.gone = true;
                    self.counts.saved += 1;
                }
            }
            State::Splatting | State::Drowning | State::Zapped => {
                if l.state_ticks >= DEATH_TICKS {
                    l.gone = true;
                    self.counts.dead += 1;
                }
            }
            State::Falling { from_y } => self.fall(l, from_y),
            State::Walking => self.walk(l),
        }
        if !matches!(l.state, State::Exiting | State::Splatting | State::Drowning | State::Zapped)
            && !self.in_bounds(l.pos)
        {
            l.set_state(State::Zapped);
        }
    }

    /// Lands a lemming on whatever is at its feet.
    fn land(&mut self, l: &mut Lemming, on_block: bool, fell: i32) {
        let ground_flags = if on_block { self.world.flags_at([l.pos[0], l.pos[1] - 1, l.pos[2]]) } else { None };
        let liquid = ground_flags.is_some_and(|f| f & flags::LIQUID != 0);
        let water = !on_block && self.world.floor(l.pos[0], l.pos[2]) == Floor::Water;
        if liquid || water {
            l.set_state(State::Drowning);
        } else if fell > SPLAT_HEIGHT && !ground_flags.is_some_and(|f| f & flags::NO_SPLAT != 0) {
            l.set_state(State::Splatting);
        } else {
            l.set_state(State::Walking);
        }
    }

    fn fall(&mut self, l: &mut Lemming, from_y: i32) {
        let target = l.pos[1] - FALL_SPEED;
        let (surface, on_block) = self.world.surface_below(l.pos[0], l.pos[1], l.pos[2]);
        if surface >= target {
            l.pos[1] = surface;
            self.land(l, on_block, from_y - surface);
        } else {
            l.pos[1] = target;
        }
    }

    fn walk(&mut self, l: &mut Lemming) {
        let d = l.dir.delta();
        // Exits: walking into an exit block through its doorway (the block's
        // +Z face, rotated with the block).
        let probe = [l.pos[0] + d[0] * REACH, l.pos[1] + SUB / 8, l.pos[2] + d[2] * REACH];
        if let Some(b) = self.world.block_at(probe)
            && b.id == EXIT_ID
        {
            let mut door = Dir::PosZ;
            for _ in 0..b.rotation {
                door = door.anticlockwise();
            }
            if l.dir == door.reverse() {
                l.set_state(State::Exiting);
                return;
            }
        }
        let next = [l.pos[0] + d[0] * WALK_SPEED, l.pos[1], l.pos[2] + d[2] * WALK_SPEED];
        let ahead = [next[0] + d[0] * REACH, next[2] + d[2] * REACH];
        // Wall: no headroom ahead, or the surface ahead is more than a step up.
        let (surface, _) = self.world.surface_below(ahead[0], l.pos[1] + STEP_UP + 1, ahead[1]);
        let blocked = surface > l.pos[1] + STEP_UP
            || self.world.solid([ahead[0], surface + HEAD_HEIGHT, ahead[1]])
            || self.world.solid([ahead[0], l.pos[1] + STEP_UP + 1, ahead[1]]);
        if blocked {
            l.dir = l.dir.reverse();
            return;
        }
        l.pos[0] = next[0];
        l.pos[2] = next[2];
        let (ground, on_block) = self.world.surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
        if ground >= l.pos[1] - STEP_DOWN {
            l.pos[1] = ground;
            if !on_block && self.world.floor(l.pos[0], l.pos[2]) == Floor::Water {
                l.set_state(State::Drowning);
            }
        } else {
            let from = l.pos[1];
            l.set_state(State::Falling { from_y: from });
        }
    }
}

/// Grid extents in sub-units, for callers that clamp cameras or cursors.
pub const EXTENT: [i32; 3] = [SIZE_X as i32 * SUB, SIZE_Y as i32 * SUB, SIZE_Z as i32 * SUB];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_turns() {
        assert_eq!(Dir::PosZ.clockwise().clockwise(), Dir::NegZ);
        assert_eq!(Dir::PosX.anticlockwise(), Dir::NegZ);
        assert_eq!(Dir::PosZ.anticlockwise().clockwise(), Dir::PosZ);
    }
}
