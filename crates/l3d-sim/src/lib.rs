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
/// Floating speed with an open umbrella (provisional).
const FLOAT_SPEED: i32 = SUB / 40;
/// Distance fallen before a floater opens the umbrella (provisional).
const FLOAT_OPEN: i32 = SUB / 2;
/// Climbing speed (provisional).
const CLIMB_SPEED: i32 = SUB / 64;
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
/// Half-size of the area around a blocker or turner that redirects walkers.
const BLOCK_RADIUS: i32 = SUB / 3;
/// Bomber fuse length (provisional: 5 seconds, as in the original Lemmings).
const FUSE_TICKS: u32 = 5 * TICKS_PER_SECOND;
/// Explosion radius in sub-units (provisional).
const BLAST_RADIUS: i32 = SUB;
/// Ticks per segment dug (provisional).
const DIG_TICKS: u32 = 12;
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

/// The skills, in the order of the original's skill panel and of the skill
/// ids in level files (`docs/spec/level.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skill {
    Blocker,
    Turner,
    Bomber,
    Builder,
    Basher,
    Miner,
    Digger,
    Climber,
    Floater,
}

impl Skill {
    pub const ALL: [Skill; 9] = [
        Skill::Blocker,
        Skill::Turner,
        Skill::Bomber,
        Skill::Builder,
        Skill::Basher,
        Skill::Miner,
        Skill::Digger,
        Skill::Climber,
        Skill::Floater,
    ];

    pub fn from_id(id: u8) -> Option<Skill> {
        Skill::ALL.get(id as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Skill::Blocker => "Blocker",
            Skill::Turner => "Turner",
            Skill::Bomber => "Bomber",
            Skill::Builder => "Builder",
            Skill::Basher => "Basher",
            Skill::Miner => "Miner",
            Skill::Digger => "Digger",
            Skill::Climber => "Climber",
            Skill::Floater => "Floater",
        }
    }
}

/// What a lemming is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Falling { from_y: i32 },
    /// Falling slowly under an umbrella.
    Floating,
    Walking,
    Climbing,
    Blocking,
    /// Standing and redirecting walkers a quarter turn.
    Turning,
    Digging,
    Exiting,
    Exploding,
    Splatting,
    Drowning,
    /// Killed by the level boundary (the original shows a lightning zap).
    Zapped,
}

impl State {
    /// Whether the lemming is finished and only playing a last animation.
    pub fn is_terminal(self) -> bool {
        matches!(self, State::Exiting | State::Exploding | State::Splatting | State::Drowning | State::Zapped)
    }
}

#[derive(Debug, Clone)]
pub struct Lemming {
    /// Feet position, sub-units.
    pub pos: [i32; 3],
    pub dir: Dir,
    pub state: State,
    /// Ticks spent in the current state.
    pub state_ticks: u32,
    pub climber: bool,
    pub floater: bool,
    /// Ticks left until a bomber explodes.
    pub fuse: Option<u32>,
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

/// A stationary lemming that redirects walkers.
#[derive(Debug, Clone, Copy)]
struct Obstacle {
    pos: [i32; 3],
    /// For turners: the heading walkers are sent off in.
    turn_to: Option<Dir>,
}

/// The state of a level being played.
pub struct Simulation {
    pub world: World,
    pub lemmings: Vec<Lemming>,
    pub entrances: Vec<Entrance>,
    /// Lemmings to release in total.
    pub to_release: u32,
    pub counts: Counts,
    /// Skills left to assign, indexed like [`Skill::ALL`].
    pub skills_left: [u32; 9],
    /// Release rate, 1..=99 (higher releases faster).
    pub release_rate: i32,
    /// The level's starting release rate; the player can't go below it.
    pub min_release_rate: i32,
    /// Ticks left on the level clock.
    pub time_left: u32,
    /// Set once the player has nuked the level.
    pub nuked: bool,
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
        let mut skills_left = [0; 9];
        for (id, n) in level.skills {
            if let Some(s) = Skill::from_id(id) {
                skills_left[s as usize] += n as u32;
            }
        }
        let b = level.border_kill;
        Simulation {
            world,
            lemmings: Vec::new(),
            entrances,
            to_release: level.lemmings as u32,
            counts: Counts::default(),
            skills_left,
            release_rate: (level.release_rate as i32).clamp(1, 99),
            min_release_rate: (level.release_rate as i32).clamp(1, 99),
            time_left: (level.time_minutes as u32 * 60 + level.time_seconds as u32) * TICKS_PER_SECOND,
            nuked: false,
            tick: 0,
            release_timer: 0,
            next_entrance: 0,
            // Stored Z first: [min z, min x, max z, max x] (docs/spec/level.md).
            border: [b[1] as i32, b[0] as i32, b[3] as i32, b[2] as i32],
            ceiling: level.ceiling_kill as i32,
        }
    }

    /// Ticks between releases (provisional: classic Lemmings timing scaled to
    /// our tick rate).
    fn release_interval(&self) -> u32 {
        let frames_17hz = (99 - self.release_rate) / 2 + 4;
        (frames_17hz as u32 * TICKS_PER_SECOND).div_ceil(17)
    }

    /// Whether `skill` can be given to lemming `i` right now.
    pub fn can_assign(&self, i: usize, skill: Skill) -> bool {
        let Some(l) = self.lemmings.get(i) else { return false };
        if l.gone || l.state.is_terminal() || self.skills_left[skill as usize] == 0 {
            return false;
        }
        let busy = matches!(l.state, State::Blocking | State::Turning);
        match skill {
            Skill::Climber => !l.climber,
            Skill::Floater => !l.floater,
            Skill::Bomber => l.fuse.is_none(),
            Skill::Blocker | Skill::Turner | Skill::Digger => l.state == State::Walking && !busy,
            // Not implemented yet.
            Skill::Builder | Skill::Basher | Skill::Miner => false,
        }
    }

    /// Gives `skill` to lemming `i`; returns whether it was accepted.
    pub fn assign(&mut self, i: usize, skill: Skill) -> bool {
        if !self.can_assign(i, skill) {
            return false;
        }
        self.skills_left[skill as usize] -= 1;
        let l = &mut self.lemmings[i];
        match skill {
            Skill::Climber => l.climber = true,
            Skill::Floater => l.floater = true,
            Skill::Bomber => l.fuse = Some(FUSE_TICKS),
            Skill::Blocker => l.set_state(State::Blocking),
            Skill::Turner => l.set_state(State::Turning),
            Skill::Digger => l.set_state(State::Digging),
            Skill::Builder | Skill::Basher | Skill::Miner => unreachable!("rejected by can_assign"),
        }
        true
    }

    /// Advances the simulation by one tick.
    pub fn step(&mut self) {
        if self.finished() {
            return;
        }
        self.tick += 1;
        self.time_left = self.time_left.saturating_sub(1);
        if self.counts.released < self.to_release && !self.entrances.is_empty() {
            if self.release_timer == 0 {
                let e = self.entrances[self.next_entrance % self.entrances.len()];
                self.next_entrance += 1;
                self.lemmings.push(Lemming {
                    pos: e.spawn,
                    dir: e.dir,
                    state: State::Falling { from_y: e.spawn[1] },
                    state_ticks: 0,
                    climber: false,
                    floater: false,
                    fuse: None,
                    gone: false,
                });
                self.counts.released += 1;
                self.release_timer = self.release_interval();
            } else {
                self.release_timer -= 1;
            }
        }
        let obstacles: Vec<Obstacle> = self
            .lemmings
            .iter()
            .filter(|l| !l.gone)
            .filter_map(|l| match l.state {
                State::Blocking => Some(Obstacle { pos: l.pos, turn_to: None }),
                // A turner sends walkers off a quarter turn clockwise from its
                // own heading (provisional).
                State::Turning => Some(Obstacle { pos: l.pos, turn_to: Some(l.dir.clockwise()) }),
                _ => None,
            })
            .collect();
        for i in 0..self.lemmings.len() {
            if !self.lemmings[i].gone {
                let mut l = self.lemmings[i].clone();
                self.update(&mut l, &obstacles);
                self.lemmings[i] = l;
            }
        }
    }

    /// Changes the release rate by `delta`, between the level's starting rate
    /// and 99.
    pub fn adjust_release_rate(&mut self, delta: i32) {
        self.release_rate = (self.release_rate + delta).clamp(self.min_release_rate, 99);
    }

    /// Stops further releases and gives every lemming in play a bomber fuse,
    /// staggered by one tick per lemming (provisional).
    pub fn nuke(&mut self) {
        if self.nuked {
            return;
        }
        self.nuked = true;
        self.to_release = self.counts.released;
        let mut stagger = 0;
        for l in self.lemmings.iter_mut().filter(|l| !l.gone && !l.state.is_terminal()) {
            if l.fuse.is_none() {
                l.fuse = Some(FUSE_TICKS + stagger);
                stagger += 1;
            }
        }
    }

    /// Whether the level is over: time ran out, or every lemming has been
    /// released and has left play.
    pub fn finished(&self) -> bool {
        self.time_left == 0 || (self.counts.released >= self.to_release && self.out() == 0)
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

    fn update(&mut self, l: &mut Lemming, obstacles: &[Obstacle]) {
        l.state_ticks += 1;
        if let Some(f) = &mut l.fuse
            && !l.state.is_terminal()
        {
            *f = f.saturating_sub(1);
            if *f == 0 {
                self.explode(l);
                return;
            }
        }
        match l.state {
            State::Exiting => {
                if l.state_ticks >= EXIT_TICKS {
                    l.gone = true;
                    self.counts.saved += 1;
                }
            }
            State::Exploding | State::Splatting | State::Drowning | State::Zapped => {
                if l.state_ticks >= DEATH_TICKS {
                    l.gone = true;
                    self.counts.dead += 1;
                }
            }
            State::Falling { from_y } => self.fall(l, from_y),
            State::Floating => self.float(l),
            State::Walking => self.walk(l, obstacles),
            State::Climbing => self.climb(l),
            State::Digging => self.dig(l),
            State::Blocking | State::Turning => {
                // Standing still; fall if the ground disappears.
                let (ground, _) = self.world.surface_below(l.pos[0], l.pos[1], l.pos[2]);
                if ground < l.pos[1] - STEP_DOWN {
                    let from = l.pos[1];
                    l.set_state(State::Falling { from_y: from });
                }
            }
        }
        if !l.state.is_terminal() && !self.in_bounds(l.pos) {
            l.set_state(State::Zapped);
        }
    }

    /// Ends a bomber: removes non-steel terrain around it.
    fn explode(&mut self, l: &mut Lemming) {
        let centre = [l.pos[0], l.pos[1] + SUB / 4, l.pos[2]];
        let r = BLAST_RADIUS;
        let cell = |v: i32| v.div_euclid(SUB);
        for cx in cell(centre[0] - r)..=cell(centre[0] + r) {
            for cy in cell(centre[1] - r)..=cell(centre[1] + r) {
                for cz in cell(centre[2] - r)..=cell(centre[2] + r) {
                    let mut mask = 0u8;
                    for seg in 0..4 {
                        // Centre of this segment slice.
                        let p = [cx * SUB + SUB / 2, cy * SUB + seg * SUB / 4 + SUB / 8, cz * SUB + SUB / 2];
                        let d = [0, 1, 2].map(|i| (p[i] - centre[i]) as i64);
                        if d.iter().map(|v| v * v).sum::<i64>() <= (r as i64) * (r as i64) {
                            mask |= 1 << seg;
                        }
                    }
                    self.world.remove_segments([cx, cy, cz], mask);
                }
            }
        }
        l.fuse = None;
        l.set_state(State::Exploding);
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
            if l.floater && from_y - target > FLOAT_OPEN {
                l.set_state(State::Floating);
            }
        }
    }

    fn float(&mut self, l: &mut Lemming) {
        let target = l.pos[1] - FLOAT_SPEED;
        let (surface, on_block) = self.world.surface_below(l.pos[0], l.pos[1], l.pos[2]);
        if surface >= target {
            l.pos[1] = surface;
            self.land(l, on_block, 0);
        } else {
            l.pos[1] = target;
        }
    }

    fn climb(&mut self, l: &mut Lemming) {
        let d = l.dir.delta();
        let ahead = [l.pos[0] + d[0] * REACH, l.pos[2] + d[2] * REACH];
        if self.world.solid([l.pos[0], l.pos[1] + HEAD_HEIGHT + CLIMB_SPEED, l.pos[2]]) {
            // Ceiling: let go and fall back.
            l.dir = l.dir.reverse();
            let from = l.pos[1];
            l.set_state(State::Falling { from_y: from });
            return;
        }
        l.pos[1] += CLIMB_SPEED;
        if !self.world.solid([ahead[0], l.pos[1], ahead[1]]) {
            // Reached the top edge: step onto it.
            l.pos[0] = ahead[0];
            l.pos[2] = ahead[1];
            let (ground, _) = self.world.surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
            l.pos[1] = ground.max(l.pos[1]);
            l.set_state(State::Walking);
        }
    }

    fn dig(&mut self, l: &mut Lemming) {
        if !l.state_ticks.is_multiple_of(DIG_TICKS) {
            return;
        }
        let c = [l.pos[0].div_euclid(SUB), (l.pos[1] - 1).div_euclid(SUB), l.pos[2].div_euclid(SUB)];
        match self.world.block(c) {
            Some((b, f)) if f & flags::STEEL == 0 => {
                // Remove the highest present segment under the feet.
                let top = (7 - b.segments.leading_zeros()).min(3);
                self.world.remove_segments(c, 1 << top);
                let (ground, on_block) = self.world.surface_below(l.pos[0], l.pos[1], l.pos[2]);
                if !on_block || ground < l.pos[1] - STEP_DOWN {
                    let from = l.pos[1];
                    l.set_state(State::Falling { from_y: from });
                } else {
                    l.pos[1] = ground;
                }
            }
            _ => l.set_state(State::Walking),
        }
    }

    fn walk(&mut self, l: &mut Lemming, obstacles: &[Obstacle]) {
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
        // Blockers and turners: walking into one redirects the walker.
        for o in obstacles {
            let near = |p: [i32; 3]| {
                (p[0] - o.pos[0]).abs() < BLOCK_RADIUS
                    && (p[2] - o.pos[2]).abs() < BLOCK_RADIUS
                    && (p[1] - o.pos[1]).abs() < SUB / 2
            };
            if near(next) && !near(l.pos) {
                l.dir = o.turn_to.unwrap_or(l.dir.reverse());
                return;
            }
        }
        let ahead = [next[0] + d[0] * REACH, next[2] + d[2] * REACH];
        // Wall: no headroom ahead, or the surface ahead is more than a step up.
        let (surface, _) = self.world.surface_below(ahead[0], l.pos[1] + STEP_UP + 1, ahead[1]);
        let blocked = surface > l.pos[1] + STEP_UP
            || self.world.solid([ahead[0], surface + HEAD_HEIGHT, ahead[1]])
            || self.world.solid([ahead[0], l.pos[1] + STEP_UP + 1, ahead[1]]);
        if blocked {
            if l.climber && self.world.solid([ahead[0], l.pos[1] + STEP_UP + 1, ahead[1]]) {
                l.set_state(State::Climbing);
            } else {
                l.dir = l.dir.reverse();
            }
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

    #[test]
    fn skill_order() {
        assert_eq!(Skill::from_id(0), Some(Skill::Blocker));
        assert_eq!(Skill::from_id(8), Some(Skill::Floater));
        assert_eq!(Skill::from_id(9), None);
    }
}
