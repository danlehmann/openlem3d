//! Deterministic Lemmings 3D simulation: lemmings moving through a level's
//! collision geometry at a fixed tick rate. No rendering or engine types.
//!
//! Behaviour constants are provisional until measured against the original
//! game; see `docs/spec/behaviour.md`.

pub mod demos;
pub mod objects;
pub mod world;

use l3d_formats::blk::{BlockSet, flags};
use l3d_formats::level::{Level, SIZE_X, SIZE_Y, SIZE_Z};
pub use world::{Brick, Floor, GROUND, SUB, World};

/// Simulation ticks per second (verified: 14 Hz, `docs/spec/behaviour.md`).
pub const TICKS_PER_SECOND: u32 = 14;

/// Walking speed, sub-units per tick (verified: 1/32 unit per tick).
const WALK_SPEED: i32 = SUB / 32;
/// Falling speed for the first ticks of a fall, then the full speed
/// (verified ±6%: about 1/6, then 1/4 unit per tick).
const FALL_SPEED_START: i32 = SUB / 6;
const FALL_SPEED: i32 = SUB / 4;
/// Ticks of a fall at the starting speed (verified: about 4).
const FALL_START_TICKS: u32 = 4;
/// Floating speed with an open umbrella (verified ±5%: the walking speed).
const FLOAT_SPEED: i32 = SUB / 32;
/// Distance fallen before a floater opens the umbrella (verified, rough:
/// about 1½ units, ~7 ticks).
const FLOAT_OPEN: i32 = 3 * SUB / 2;
/// Climbing speed (rough measurement: about 0.04 unit per tick).
const CLIMB_SPEED: i32 = SUB / 25;
/// Highest step a walker climbs without turning (provisional: one segment).
const STEP_UP: i32 = SUB / 4;
/// Lowest drop a walker steps down without falling (provisional).
const STEP_DOWN: i32 = SUB / 4;
/// Falls longer than this splat (bracketed: 2 units survive, 4¼ splat).
const SPLAT_HEIGHT: i32 = 4 * SUB;
/// Clearance a walker needs above the surface it walks onto: openings exactly
/// this high (half-height gaps under a lintel) are passable (verified: the
/// half-unit doorways of Practice "Claustrophobic", `LEVEL.090`).
const HEAD_HEIGHT: i32 = SUB / 2;
/// How far ahead of its centre a walker probes for walls.
const REACH: i32 = SUB / 4;
/// Half-size of the area around a blocker or turner that redirects walkers
/// (verified along the path: 0.48 ± 0.05 unit; across the path assumed equal).
const BLOCK_RADIUS: i32 = 123;
/// Ticks per countdown digit shown over a bomber (verified: about 8).
pub const FUSE_DIGIT_TICKS: u32 = 8;
/// Bomber fuse length from assignment to explosion (verified: about 3.6 s,
/// a 5…1 countdown of 8-tick digits followed by the swelling animation).
pub const FUSE_TICKS: u32 = 50;
/// Explosion radius in sub-units (rough: the hole is about one cell long).
const BLAST_RADIUS: i32 = SUB;
/// Ticks per segment dug (verified, rough: about 40 ±20%).
const DIG_TICKS: u32 = 40;
/// Ticks per brick laid by a builder (verified: 25 ± 1).
pub const BUILD_TICKS: u32 = 25;
/// Bricks per builder (verified: 6).
const BRICKS: u8 = 6;
/// Each brick moves the builder this far forward and ¼ unit up (verified
/// ±10%); bricks reach from [`BRICK_START`] to [`BRICK_END`] ahead of the
/// feet, about 0.55 unit long (measured), so consecutive bricks overlap.
const BRICK_RUN: i32 = SUB / 2;
const BRICK_START: i32 = SUB / 8;
const BRICK_END: i32 = SUB / 2 + SUB / 8 + SUB / 16;
/// Ticks per stroke of a basher (verified, rough: 30–34) and of a miner
/// (rough: about 3.5 s).
const BASH_TICKS: u32 = 32;
const MINE_TICKS: u32 = 49;
/// Duration of terminal animations, in ticks (provisional).
const EXIT_TICKS: u32 = 11;
const DEATH_TICKS: u32 = 14;
/// How long a killing trap stays busy after taking a lemming (provisional).
pub const TRAP_BUSY_TICKS: u32 = 28;
/// How long a spring shows its launch after throwing a lemming (unmeasured:
/// one tick per frame of its seven-frame animation).
pub const SPRING_BUSY_TICKS: u32 = 7;
/// Rope-slide speed, sub-units per tick (unmeasured).
const ROPE_SPEED: i32 = 35;
/// Ticks a lemming hangs from the rope's handle before sliding (observed:
/// about 0.6 s; with [`ROPE_SPEED`], Practice "Rope Slide" takes about
/// 6.35 s from grab to release, as observed).
const ROPE_GRAB_TICKS: u32 = 8;
/// Spring flights: ticks per unit of horizontal distance (observed: about
/// 1.6 s for 16 units in Practice "Catapults"), and peak height as a
/// fraction of the distance (unmeasured).
const SPRING_TICKS_PER_UNIT: f32 = 1.4;
const SPRING_ARC: f32 = 0.3;
/// Trampoline flight, per tick and sub-units, from the original's Practice
/// "Trampoline" demo: gravity (about 8.5 units/s²), forward speed (1.48
/// units/s), the first hop's take-off speed (peaking 1.5 units up), the
/// growth per bounce (the second hop peaked about 2.4 units up) and the
/// strongest bounce (peaking 7.5 units above a pad). The last two were
/// chosen within the measurements so that, as in the demo, every lemming
/// lands on that level's floating cube and the demo's one turner saves 18.
const BOUNCE_GRAVITY: i32 = 11;
const BOUNCE_VX: i32 = 27;
const BOUNCE_FIRST_VY: i32 = 92;
const BOUNCE_GAIN: f32 = 1.32;
const BOUNCE_MAX_VY: i32 = 205;
/// Drops onto a trampoline longer than this (sub-units) are thrown far.
const BIG_DROP: i32 = 8 * SUB;
/// Speed on slippery blocks, sub-units per tick.
const ICE_SPEED: i32 = WALK_SPEED * 3;

/// Block ids with hard-coded meaning (`docs/spec/blk.md`).
const ENTRANCE_ID: u8 = 0;
const EXIT_ID: u8 = 1;
const SPLITTER_ID: u8 = 2;

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
    /// Standing and pointing one arm sideways; walkers that reach it leave in
    /// the direction it points.
    Turning { to: Dir },
    Digging,
    /// Laying steps; the count is the bricks still to lay.
    Building { bricks_left: u8 },
    /// Removing terrain straight ahead.
    Bashing,
    /// Removing terrain diagonally ahead and down.
    Mining,
    Exiting,
    Exploding,
    Splatting,
    Drowning,
    /// Killed by the level boundary (the original shows a lightning zap).
    Zapped,
    /// Caught by a trap; the trap's own animation shows the death.
    Trapped,
    /// Hanging from a rope slide, riding towards its far end (feet
    /// position, sub-units).
    OnRope { to: [i32; 3] },
    /// Walking on a slippery block: faster, and no skill that needs a
    /// standing lemming can be given (the original shows "Sliding").
    Sliding,
    /// Bouncing on trampolines: rising (or falling, if negative) at `vy`
    /// sub-units per tick while moving forward.
    Bouncing { vy: i32, vx: i32 },
    /// Thrown through the air by a spring from `from` to
    /// `to` (feet positions) over `ticks` ticks, rising `peak`
    /// sub-units above the straight line at mid-flight.
    Flying { from: [i32; 3], to: [i32; 3], ticks: u32, peak: i32 },
    /// Standing on a teleporter pad, vanishing, away, and reappearing on
    /// the partner pad `to` (feet position); see [`TELEPORT_TICKS`].
    Teleporting { to: [i32; 3] },
}

/// Teleporting, in ticks from stopping on the pad centre (observed in the
/// original's Practice "Teleporter" demo, about 2 s in all): stands, is
/// squeezed to nothing, is away while sparkles show at both pads, stretches
/// out again on the partner, stands, and walks on.
pub const TELEPORT_STAND: u32 = 4;
pub const TELEPORT_VANISH: u32 = 3;
pub const TELEPORT_AWAY: u32 = 12;
pub const TELEPORT_APPEAR: u32 = 4;
pub const TELEPORT_TICKS: u32 = 2 * TELEPORT_STAND + TELEPORT_VANISH + TELEPORT_AWAY + TELEPORT_APPEAR;

impl State {
    /// Whether the lemming is finished and only playing a last animation.
    pub fn is_terminal(self) -> bool {
        matches!(self, State::Exiting | State::Exploding | State::Splatting | State::Drowning | State::Zapped | State::Trapped)
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
    /// Standing on the teleporter it arrived on; it must step off before
    /// another teleport.
    pub teleported: bool,
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

/// Something a lemming did that the player hears (`docs/spec/sound.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Exited,
    Drowned,
    Splatted,
    Exploded,
    Zapped,
    Trapped,
    Climbed,
    UmbrellaOpened,
    Bounced,
    Catapulted,
    Teleported,
    Brick,
    BuilderDone,
    Dug,
    Bashed,
    Mined,
}

/// A stationary lemming that redirects walkers.
/// Something the player did to a running level. With the tick it happened
/// on (after that many steps), a list of these replays a game exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Assign { lemming: usize, skill: Skill },
    Turner { lemming: usize, to: Dir },
    ReleaseRate(i32),
    Nuke,
}

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
    /// Every accepted command so far, with the tick it was given on.
    pub log: Vec<(u64, Command)>,
    /// Ticks until the next release.
    release_timer: u32,
    next_entrance: usize,
    /// Splitter cells, and whether the next walker through goes left.
    splitters: Vec<([i32; 3], bool)>,
    /// Events since the last [`Simulation::take_events`].
    events: Vec<Event>,
    /// The level's interactive objects and what they all are.
    pub object_kind: Option<objects::ObjectKind>,
    pub objects: Vec<objects::Object>,
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
                // Release direction by rotation: 0 → −Z, 1 → +X, 2 → +Z,
                // 3 → −X, i.e. clockwise steps from −Z (verified in the
                // original for all four rotations; [L3DEdit]'s +Z start is
                // wrong).
                let mut dir = Dir::NegZ;
                for _ in 0..b.rotation {
                    dir = dir.clockwise();
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
            log: Vec::new(),
            release_timer: 0,
            next_entrance: 0,
            events: Vec::new(),
            object_kind: objects::level_objects(level).map(|(k, _)| k),
            objects: objects::level_objects(level).map(|(_, o)| o).unwrap_or_default(),
            splitters: level
                .cells()
                .filter(|(_, _, _, b, _)| b.id == SPLITTER_ID && !b.is_empty())
                .map(|(x, y, z, _, _)| ([x as i32, y as i32, z as i32], false))
                .collect(),
            // Stored Z first: [min z, min x, max z, max x] (docs/spec/level.md).
            border: [b[1] as i32, b[0] as i32, b[3] as i32, b[2] as i32],
            ceiling: level.ceiling_kill as i32,
        }
    }

    /// Ticks between releases (verified: `101 − rate`).
    fn release_interval(&self) -> u32 {
        (101 - self.release_rate) as u32
    }

    /// Whether `skill` can be given to lemming `i` right now.
    pub fn can_assign(&self, i: usize, skill: Skill) -> bool {
        let Some(l) = self.lemmings.get(i) else { return false };
        if l.gone || l.state.is_terminal() || self.skills_left[skill as usize] == 0 {
            return false;
        }
        let busy = matches!(l.state, State::Blocking | State::Turning { .. });
        match skill {
            Skill::Climber => !l.climber,
            Skill::Floater => !l.floater,
            Skill::Bomber => l.fuse.is_none(),
            Skill::Blocker
            | Skill::Turner
            | Skill::Digger
            | Skill::Builder
            | Skill::Basher
            | Skill::Miner => l.state == State::Walking && !busy,
        }
    }

    /// Gives `skill` to lemming `i`; returns whether it was accepted.
    pub fn assign(&mut self, i: usize, skill: Skill) -> bool {
        let ok = self.give(i, skill);
        if ok {
            self.log.push((self.tick, Command::Assign { lemming: i, skill }));
        }
        ok
    }

    fn give(&mut self, i: usize, skill: Skill) -> bool {
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
            // The turner's own left (the one verified case; see `assign_turner`).
            Skill::Turner => l.set_state(State::Turning { to: l.dir.anticlockwise() }),
            Skill::Digger => l.set_state(State::Digging),
            Skill::Builder => l.set_state(State::Building { bricks_left: BRICKS }),
            Skill::Basher => l.set_state(State::Bashing),
            Skill::Miner => l.set_state(State::Mining),
        }
        true
    }

    /// Makes lemming `i` a turner pointing `to`, a quarter turn either way
    /// from its heading. In the original the player picks the side with a
    /// second click (`docs/spec/behaviour.md`, "A turner takes two clicks").
    pub fn assign_turner(&mut self, i: usize, to: Dir) -> bool {
        let Some(l) = self.lemmings.get(i) else { return false };
        if to == l.dir || to == l.dir.reverse() || !self.give(i, Skill::Turner) {
            return false;
        }
        self.lemmings[i].state = State::Turning { to };
        self.log.push((self.tick, Command::Turner { lemming: i, to }));
        true
    }

    /// Carries out a command; returns whether it was accepted.
    pub fn apply(&mut self, c: Command) -> bool {
        match c {
            Command::Assign { lemming, skill } => self.assign(lemming, skill),
            Command::Turner { lemming, to } => self.assign_turner(lemming, to),
            Command::ReleaseRate(delta) => {
                self.adjust_release_rate(delta);
                true
            }
            Command::Nuke => {
                let first = !self.nuked;
                self.nuke();
                first
            }
        }
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
                    teleported: false,
                });
                self.counts.released += 1;
                // The countdown includes the releasing tick: the next release
                // comes `release_interval` ticks after this one.
                self.release_timer = self.release_interval() - 1;
            } else {
                self.release_timer -= 1;
            }
        }
        for o in &mut self.objects {
            o.busy = o.busy.saturating_sub(1);
        }
        let obstacles: Vec<Obstacle> = self
            .lemmings
            .iter()
            .filter(|l| !l.gone)
            .filter_map(|l| match l.state {
                State::Blocking => Some(Obstacle { pos: l.pos, turn_to: None }),
                State::Turning { to } => Some(Obstacle { pos: l.pos, turn_to: Some(to) }),
                _ => None,
            })
            .collect();
        for i in 0..self.lemmings.len() {
            if !self.lemmings[i].gone {
                let mut l = self.lemmings[i].clone();
                self.update(&mut l, &obstacles);
                let before = &self.lemmings[i];
                if let Some(e) = self.event_for(before, &l) {
                    self.events.push(e);
                }
                self.lemmings[i] = l;
            }
        }
    }

    /// What a lemming's update did that a player would hear.
    fn event_for(&self, before: &Lemming, after: &Lemming) -> Option<Event> {
        let (b, a) = (before.state, after.state);
        let changed = std::mem::discriminant(&a) != std::mem::discriminant(&b) || after.state_ticks == 0;
        if !before.teleported && after.teleported {
            return Some(Event::Teleported);
        }
        if changed {
            return match a {
                State::Exiting => Some(Event::Exited),
                State::Drowning => Some(Event::Drowned),
                State::Splatting => Some(Event::Splatted),
                State::Exploding => Some(Event::Exploded),
                State::Zapped => Some(Event::Zapped),
                State::Trapped => Some(Event::Trapped),
                State::Climbing => Some(Event::Climbed),
                State::Floating => Some(Event::UmbrellaOpened),
                State::Bouncing { .. } => Some(Event::Bounced),
                State::Flying { .. } => Some(Event::Catapulted),
                State::Walking if matches!(b, State::Building { .. }) => Some(Event::BuilderDone),
                _ => None,
            };
        }
        let on = |n: u32| after.state_ticks.is_multiple_of(n);
        match a {
            State::Building { bricks_left } if matches!(b, State::Building { bricks_left: was } if was > bricks_left) => Some(Event::Brick),
            State::Digging if on(DIG_TICKS) => Some(Event::Dug),
            State::Bashing if on(BASH_TICKS) => Some(Event::Bashed),
            State::Mining if on(MINE_TICKS) => Some(Event::Mined),
            _ => None,
        }
    }

    /// Drains the events since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Changes the release rate by `delta`, between the level's starting rate
    /// and 99.
    pub fn adjust_release_rate(&mut self, delta: i32) {
        let rate = (self.release_rate + delta).clamp(self.min_release_rate, 99);
        if rate != self.release_rate {
            self.log.push((self.tick, Command::ReleaseRate(rate - self.release_rate)));
            self.release_rate = rate;
        }
    }

    /// Stops further releases and gives every lemming in play a bomber fuse,
    /// staggered by one tick per lemming (provisional).
    pub fn nuke(&mut self) {
        if self.nuked {
            return;
        }
        self.nuked = true;
        self.log.push((self.tick, Command::Nuke));
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
            State::Trapped => {
                if l.state_ticks >= TRAP_BUSY_TICKS {
                    l.gone = true;
                    self.counts.dead += 1;
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
            State::Walking | State::Sliding => self.walk(l, obstacles),
            State::Bouncing { vy, vx } => self.fly(l, vy, vx),
            State::Climbing => self.climb(l),
            State::Digging => self.dig(l),
            State::Building { bricks_left } => self.build(l, bricks_left),
            State::Bashing => self.bash(l),
            State::Mining => self.mine(l),
            // The lemming first hangs on the handle for a moment (observed).
            State::OnRope { .. } if l.state_ticks <= ROPE_GRAB_TICKS => {}
            State::Teleporting { to } => {
                if l.state_ticks == TELEPORT_STAND + TELEPORT_VANISH + TELEPORT_AWAY {
                    l.pos = to;
                    l.teleported = true;
                }
                if l.state_ticks >= TELEPORT_TICKS {
                    l.set_state(State::Walking);
                }
            }
            State::OnRope { to } => {
                let d = [0, 1, 2].map(|i| to[i] - l.pos[i]);
                let len = ((d[0] as i64).pow(2) + (d[1] as i64).pow(2) + (d[2] as i64).pow(2)).isqrt() as i32;
                if len <= ROPE_SPEED {
                    l.pos = to;
                    l.set_state(State::Walking);
                    self.settle(l);
                } else {
                    for (p, d) in l.pos.iter_mut().zip(d) {
                        *p += d * ROPE_SPEED / len;
                    }
                }
            }
            State::Flying { from, to, ticks, peak } => {
                let f = (l.state_ticks as f32 / ticks as f32).min(1.0);
                let lerp = |i: usize| from[i] + ((to[i] - from[i]) as f32 * f) as i32;
                let arc = (peak as f32 * 4.0 * f * (1.0 - f)) as i32;
                l.pos = [lerp(0), lerp(1) + arc, lerp(2)];
                if f >= 1.0 {
                    l.pos = to;
                    l.set_state(State::Walking);
                    self.settle(l);
                }
            }
            State::Blocking | State::Turning { .. } => {
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
        self.spring_traps(l);
        self.teleport(l);
        self.board_rope_slide(l);
        self.spring(l);
    }

    /// Killing traps take a lemming that steps on them, then are busy for a
    /// while, letting others pass (provisional, as in the 2D games).
    fn spring_traps(&mut self, l: &mut Lemming) {
        if !self.object_kind.is_some_and(|k| k.kills()) || l.state.is_terminal() || matches!(l.state, State::Falling { .. } | State::Floating) {
            return;
        }
        if let Some(o) = self.objects.iter_mut().find(|o| o.busy == 0 && o.touches(l.pos)) {
            o.busy = TRAP_BUSY_TICKS;
            l.fuse = None;
            l.set_state(State::Trapped);
        }
    }

    /// Teleporters: a walker reaching the centre of one stops there and,
    /// after [`TELEPORT_TICKS`], walks on in the same direction from its
    /// partner (the other object with the same value; with more, the last
    /// two pair up [L3DEdit]).
    fn teleport(&mut self, l: &mut Lemming) {
        if self.object_kind != Some(objects::ObjectKind::Teleporter) || !matches!(l.state, State::Walking | State::Sliding) {
            return;
        }
        let Some(here) = self.objects.iter().position(|o| o.touches(l.pos)) else {
            l.teleported = false;
            return;
        };
        if l.teleported {
            return;
        }
        let value = self.objects[here].value;
        let pair: Vec<usize> = self.objects.iter().enumerate().filter(|(_, o)| o.value == value).map(|(i, _)| i).collect();
        let [.., a, b] = pair[..] else { return };
        let to = match here {
            h if h == a => b,
            h if h == b => a,
            _ => return,
        };
        // Wait for the centre of the pad.
        let pad = self.objects[here];
        let centre = [pad.cell[0] * SUB + SUB / 2, pad.cell[2] * SUB + SUB / 2];
        let d = l.dir.delta();
        if (centre[0] - l.pos[0]) * d[0] + (centre[1] - l.pos[2]) * d[2] > 0 {
            return;
        }
        l.pos[0] = centre[0];
        l.pos[2] = centre[1];
        let o = self.objects[to];
        l.set_state(State::Teleporting { to: [o.cell[0] * SUB + SUB / 2, o.surface, o.cell[2] * SUB + SUB / 2] });
    }

    /// Rope slides: a lemming reaching a sender (an even value) rides to its
    /// receiver (the next value up); with several of a value only the last
    /// works [L3DEdit]. Provisional: a straight line at [`ROPE_SPEED`],
    /// then it walks on in its old direction.
    fn board_rope_slide(&mut self, l: &mut Lemming) {
        if self.object_kind != Some(objects::ObjectKind::RopeSlide) || !matches!(l.state, State::Walking | State::Sliding | State::Falling { .. }) {
            return;
        }
        let last = |v: u8| self.objects.iter().rposition(|o| o.value == v);
        let Some(sender) = self.objects.iter().position(|o| o.value % 2 == 0 && o.touches(l.pos)) else { return };
        let value = self.objects[sender].value;
        if last(value) != Some(sender) {
            return;
        }
        let Some(receiver) = last(value + 1) else { return };
        let o = self.objects[receiver];
        l.set_state(State::OnRope { to: [o.cell[0] * SUB + SUB / 2, o.surface, o.cell[2] * SUB + SUB / 2] });
    }

    /// Springs: a walker stepping onto a sender (an even value) is thrown in
    /// an arc onto its receiver (the next value up; the last of each value
    /// works [L3DEdit]) and lands unhurt, walking on in its old direction.
    /// Flight time and arc height are unmeasured.
    fn spring(&mut self, l: &mut Lemming) {
        if self.object_kind != Some(objects::ObjectKind::Spring) || !matches!(l.state, State::Walking | State::Sliding) {
            return;
        }
        let last = |v: u8| self.objects.iter().rposition(|o| o.value == v);
        let Some(sender) = self.objects.iter().position(|o| o.value % 2 == 0 && o.touches(l.pos)) else { return };
        let value = self.objects[sender].value;
        let (Some(receiver), true) = (last(value + 1), last(value) == Some(sender)) else { return };
        let o = self.objects[receiver];
        let to = [o.cell[0] * SUB + SUB / 2, o.surface, o.cell[2] * SUB + SUB / 2];
        let span = ((to[0] - l.pos[0]) as f32).hypot((to[2] - l.pos[2]) as f32) / SUB as f32;
        let ticks = ((span * SPRING_TICKS_PER_UNIT) as u32).max(8);
        let peak = (span * SPRING_ARC * SUB as f32) as i32;
        self.objects[sender].busy = SPRING_BUSY_TICKS;
        l.set_state(State::Flying { from: l.pos, to, ticks, peak });
    }

    /// Trampolines: a lemming arriving on a pad is thrown up and forward and
    /// flies ballistically ([`Simulation::fly`]); each bounce is stronger
    /// than the last, and a drop onto a pad bounces back as high as it fell.
    /// Measured from the original's Practice "Trampoline" demo: forward
    /// speed about 1.47 units/s, first hop peaking about 1.5 units up
    /// (1.2 s), the second about 2.4 (1.45 s), gravity about 8.5 units/s².
    /// Returns whether it bounced.
    fn bounce(&mut self, l: &mut Lemming, height: i32) -> bool {
        if self.object_kind != Some(objects::ObjectKind::Trampoline) || !self.objects.iter().any(|o| o.touches(l.pos)) {
            return false;
        }
        let from_drop = ((2 * BOUNCE_GRAVITY * height.max(0)) as f32).sqrt() as i32;
        let (vy, vx) = if height > BIG_DROP {
            // A long drop throws the lemming back as high as it fell and
            // forward about 0.8 of that (provisional: Fun 1 drops lemmings 12
            // units onto a trampoline that must carry them about 10 units to
            // the exit; the demo hints that stronger bounces move faster).
            let ticks = 2 * from_drop / BOUNCE_GRAVITY;
            (from_drop, (height * 4 / 5 / ticks.max(1)).max(BOUNCE_VX))
        } else {
            (from_drop.clamp(BOUNCE_FIRST_VY, BOUNCE_MAX_VY), BOUNCE_VX)
        };
        l.set_state(State::Bouncing { vy, vx });
        true
    }

    /// One tick of a bouncing lemming: forward at a steady speed, up or down
    /// under gravity. Walls turn it back and deflectors turn it aside in the
    /// air; on a pad it bounces again, higher; elsewhere it lands unhurt.
    fn fly(&mut self, l: &mut Lemming, vy: i32, vx: i32) {
        let d = l.dir.delta();
        let next = [l.pos[0] + d[0] * vx, l.pos[1] + vy, l.pos[2] + d[2] * vx];
        let body = next[1] + SUB / 8;
        if let Some(n) = self.deflector_across(next[0], next[1], next[2]).filter(|n| d[0] * n[0] + d[2] * n[1] < 0) {
            let k = d[0] * n[0] + d[2] * n[1];
            l.dir = match [d[0] - k * n[0], d[2] - k * n[1]] {
                [1, 0] => Dir::PosX,
                [-1, 0] => Dir::NegX,
                [0, 1] => Dir::PosZ,
                _ => Dir::NegZ,
            };
            l.state = State::Bouncing { vy: vy - BOUNCE_GRAVITY, vx };
            return;
        }
        let wall = |w: &World, y: i32| w.solid([next[0], y, next[2]]) && !w.solid([l.pos[0], y, l.pos[2]]);
        if vy >= 0 && (wall(&self.world, body) || wall(&self.world, next[1] + HEAD_HEIGHT - 1)) {
            l.dir = l.dir.reverse();
            l.state = State::Bouncing { vy: vy - BOUNCE_GRAVITY, vx };
            return;
        }
        if vy < 0 {
            let (ground, on_block) = self.world.surface_below(next[0], l.pos[1] + STEP_UP, next[2]);
            // Terrain at body height ahead (a slope's side) turns it back.
            if self.world.solid([next[0], ground + STEP_UP + 1, next[2]]) || ground > l.pos[1] + STEP_UP {
                l.dir = l.dir.reverse();
                l.state = State::Bouncing { vy: vy - BOUNCE_GRAVITY, vx };
                return;
            }
            if ground >= next[1] {
                l.pos = [next[0], ground, next[2]];
                if self.object_kind == Some(objects::ObjectKind::Trampoline) && self.objects.iter().any(|o| o.touches(l.pos)) {
                    let up = ((-vy) as f32 * BOUNCE_GAIN) as i32;
                    l.set_state(State::Bouncing { vy: up.min(BOUNCE_MAX_VY), vx: BOUNCE_VX });
                } else {
                    self.land(l, on_block, 0);
                }
                return;
            }
            if wall(&self.world, body) {
                l.dir = l.dir.reverse();
                l.state = State::Bouncing { vy: vy - BOUNCE_GRAVITY, vx };
                return;
            }
        } else if self.world.solid([next[0], next[1] + HEAD_HEIGHT, next[2]]) {
            // A ceiling stops the rise.
            l.state = State::Bouncing { vy: 0, vx };
            return;
        }
        l.pos = next;
        l.state = State::Bouncing { vy: vy - BOUNCE_GRAVITY, vx };
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
        if self.bounce(l, fell) {
            return;
        }
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
        let speed = if l.state_ticks <= FALL_START_TICKS { FALL_SPEED_START } else { FALL_SPEED };
        let target = l.pos[1] - speed;
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

    /// The cell containing a sub-unit point.
    fn cell_of(p: [i32; 3]) -> [i32; 3] {
        p.map(|v| v.div_euclid(SUB))
    }

    /// Falls or stands depending on the ground now under the feet.
    fn settle(&mut self, l: &mut Lemming) -> bool {
        let (ground, _) = self.world.surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
        if ground < l.pos[1] - STEP_DOWN {
            let from = l.pos[1];
            l.set_state(State::Falling { from_y: from });
            false
        } else {
            l.pos[1] = ground;
            true
        }
    }

    /// Lays a step of one segment's height in the cell ahead, then climbs
    /// onto it (provisional: one segment per cell, inheriting the block id
    /// of the cell stood on).
    fn build(&mut self, l: &mut Lemming, bricks_left: u8) {
        if !l.state_ticks.is_multiple_of(BUILD_TICKS) {
            return;
        }
        if bricks_left == 0 {
            l.set_state(State::Walking);
            return;
        }
        let d = l.dir.delta();
        let template = match self.world.block(Self::cell_of([l.pos[0], l.pos[1] - 1, l.pos[2]])) {
            Some((b, _)) if b.id >= 9 => b.id,
            _ => self.world.common_id,
        };
        // A brick from just ahead of the feet to just past the next standing
        // point, one segment thick, a unit wide (width unmeasured).
        let along = |from: i32, to: i32, axis: usize| {
            let (a, b) = (l.pos[axis] + d[axis] * from, l.pos[axis] + d[axis] * to);
            if d[axis] == 0 { (l.pos[axis] - SUB / 2, l.pos[axis] + SUB / 2) } else { (a.min(b), a.max(b)) }
        };
        let next = [l.pos[0] + d[0] * BRICK_RUN, l.pos[1] + SUB / 4, l.pos[2] + d[2] * BRICK_RUN];
        let head_blocked = [next[1] + 1, next[1] + HEAD_HEIGHT - 1].iter().any(|&y| self.world.solid([next[0], y, next[2]]));
        // A brick that would run into terrain is cut short at it, filling
        // the gap up to the wall; the builder then stops (provisional).
        let mut end = BRICK_END;
        let laid = loop {
            let (x0, x1) = along(BRICK_START, end, 0);
            let (z0, z1) = along(BRICK_START, end, 2);
            let brick = world::Brick { min: [x0, l.pos[1], z0], max: [x1, l.pos[1] + SUB / 4, z1], id: template };
            if !head_blocked && self.world.add_brick(brick) {
                break Some(end);
            }
            end -= 1;
            if end - BRICK_START < SUB / 8 {
                break None;
            }
        };
        if laid != Some(BRICK_END) || head_blocked {
            l.set_state(State::Walking);
            return;
        }        // Climb onto the new brick.
        l.pos = next;
        l.state = State::Building { bricks_left: bricks_left - 1 };
    }
    /// Removes body-height terrain in the cell ahead and moves into the gap;
    /// stops when nothing is left to bash (provisional).
    fn bash(&mut self, l: &mut Lemming) {
        if !l.state_ticks.is_multiple_of(BASH_TICKS) {
            return;
        }
        let d = l.dir.delta();
        let ahead = [l.pos[0] + d[0] * (SUB / 2), l.pos[1], l.pos[2] + d[2] * (SUB / 2)];
        let cell = Self::cell_of(ahead);
        // Segments from the feet up to about head height.
        let first = ((l.pos[1] - cell[1] * SUB) / (SUB / 4)).clamp(0, 3);
        // The lower half-unit (two segments) from the feet up (verified, rough).
        let mask = (0b11u8 << first) & 0xF;
        match self.world.block(cell) {
            Some((b, f)) if b.segments & mask != 0 => {
                if f & flags::STEEL != 0 {
                    l.set_state(State::Walking);
                    return;
                }
                if !self.world.remove_segments_towards(cell, mask, Some(l.dir.delta())) {
                    // A one-way block facing the other way.
                    l.set_state(State::Walking);
                    return;
                }
                l.pos[0] += d[0] * (SUB / 4);
                l.pos[2] += d[2] * (SUB / 4);
                self.settle(l);
            }
            _ => l.set_state(State::Walking),
        }
    }

    /// Removes the terrain ahead at and below the feet, stepping forward and
    /// down a segment per stroke (provisional).
    fn mine(&mut self, l: &mut Lemming) {
        if !l.state_ticks.is_multiple_of(MINE_TICKS) {
            return;
        }
        let d = l.dir.delta();
        // One swing moves the miner a quarter unit forward and down (45°),
        // clearing the space its body needs there: from the new feet up to
        // above head height, from the feet to past the walker's reach.
        let feet = l.pos[1] - SUB / 4;
        if feet < GROUND {
            // The level bottom can't be mined.
            l.set_state(State::Walking);
            return;
        }
        let (y0, y1) = (feet, feet + HEAD_HEIGHT + STEP_UP);
        let (h0, h1) = (0, SUB / 4 + REACH + WALK_SPEED);
        let mut removed = false;
        for along in (h0..=h1).step_by((SUB / 8) as usize) {
            let p = [l.pos[0] + d[0] * along, l.pos[2] + d[2] * along];
            let mut cells: Vec<([i32; 3], u8)> = Vec::new();
            for y in (y0..y1).step_by((SUB / 4) as usize) {
                let c = Self::cell_of([p[0], y, p[1]]);
                let seg = (y - c[1] * SUB) / (SUB / 4);
                match cells.iter_mut().find(|(k, _)| *k == c) {
                    Some((_, m)) => *m |= 1 << seg,
                    None => cells.push((c, 1 << seg)),
                }
            }
            for (c, mask) in cells {
                let Some((b, f)) = self.world.block(c) else { continue };
                if b.segments & mask == 0 {
                    continue;
                }
                if f & flags::STEEL != 0 || !self.world.remove_segments_towards(c, mask, Some(d)) {
                    // Steel, or a one-way block facing the other way.
                    l.set_state(State::Walking);
                    return;
                }
                removed = true;
            }
        }
        if !removed && self.world.solid([l.pos[0], l.pos[1] - 1, l.pos[2]]) {
            // Nothing left to mine but ground ahead: walk on.
            l.set_state(State::Walking);
            return;
        }
        l.pos[0] += d[0] * (SUB / 4);
        l.pos[2] += d[2] * (SUB / 4);
        // Stand on whatever is left below; out in the open, fall.
        self.settle(l);
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

    /// The outward normal of a deflector face crossing the body of a lemming
    /// standing at `(x, feet, z)`, anywhere from its feet to its head (a
    /// deflector may hang above the floor).
    fn deflector_across(&self, x: i32, feet: i32, z: i32) -> Option<[i32; 2]> {
        [SUB / 8, HEAD_HEIGHT / 2, HEAD_HEIGHT - 1].into_iter().find_map(|h| self.world.deflector_normal([x, feet + h, z]))
    }

    fn walk(&mut self, l: &mut Lemming, obstacles: &[Obstacle]) {
        let d = l.dir.delta();
        // Exits: walking into an exit block through its doorway (the block's
        // +Z face, rotated with the block).
        // Probe as far ahead as the wall check below (one step plus REACH), so a
        // doorway is recognised before the walker would bounce off it.
        // On ice lemmings move about 3× as fast (observed: 3.1 ± 0.3).
        let speed = if l.state == State::Sliding { ICE_SPEED } else { WALK_SPEED };
        let reach = REACH + speed;
        let probe = [l.pos[0] + d[0] * reach, l.pos[1] + SUB / 8, l.pos[2] + d[2] * reach];
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
        let next = [l.pos[0] + d[0] * speed, l.pos[1], l.pos[2] + d[2] * speed];
        // Blockers and turners: walking into one redirects the walker.
        // Splitters send walkers alternately left and right as they cross
        // the cell centre ([L3DEdit]: block 2 where non-solid). The first goes
        // right: in the original's demo of Practice "Splitter" the first
        // lemming seen went to its right and the next to its left (weak:
        // maybe not the very first).
        let here = Self::cell_of([l.pos[0], l.pos[1], l.pos[2]]);
        if let Some(s) = self.splitters.iter_mut().find(|(c, _)| *c == here) {
            let axis = if d[0] != 0 { 0 } else { 2 };
            let centre = here[axis] * SUB + SUB / 2;
            if (centre - l.pos[axis]) * d[axis] > 0 && (centre - next[axis]) * d[axis] <= 0 {
                l.pos[axis] = centre;
                l.dir = if s.1 { l.dir.anticlockwise() } else { l.dir.clockwise() };
                s.1 = !s.1;
                return;
            }
        }
        for o in obstacles {
            let near = |p: [i32; 3]| {
                (p[0] - o.pos[0]).abs() < BLOCK_RADIUS
                    && (p[2] - o.pos[2]).abs() < BLOCK_RADIUS
                    && (p[1] - o.pos[1]).abs() < SUB / 2
            };
            match o.turn_to {
                // Blockers turn walkers back at arm's length.
                None if near(next) && !near(l.pos) => {
                    l.dir = l.dir.reverse();
                    return;
                }
                // Turners send walkers off along the turner's own line: a
                // walker turns once it is level with the turner, so it
                // leaves on the path the turner stands on (provisional).
                Some(to) if near(next) && to != l.dir => {
                    let axis = if d[0] != 0 { 0 } else { 2 };
                    let before = (o.pos[axis] - l.pos[axis]) * d[axis];
                    let after = (o.pos[axis] - next[axis]) * d[axis];
                    if before > 0 && after <= 0 {
                        l.pos[axis] = o.pos[axis];
                        l.dir = to;
                        return;
                    }
                }
                _ => {}
            }
        }
        let ahead = [next[0] + d[0] * REACH, next[2] + d[2] * REACH];
        // Deflectors: a walker meeting the diagonal face of a 45° wall turns a
        // quarter, as if reflected, once its centre reaches the face
        // (provisional). Their square backs are ordinary walls.
        let facing = |n: [i32; 2]| d[0] * n[0] + d[2] * n[1] < 0;
        if let Some(n) = self.deflector_across(ahead[0], l.pos[1], ahead[1]).filter(|&n| facing(n)) {
            if self.deflector_across(next[0], l.pos[1], next[2]).is_some() {
                let k = d[0] * n[0] + d[2] * n[1];
                let out = [d[0] - k * n[0], d[2] - k * n[1]];
                l.dir = match out {
                    [1, 0] => Dir::PosX,
                    [-1, 0] => Dir::NegX,
                    [0, 1] => Dir::PosZ,
                    _ => Dir::NegZ,
                };
                return;
            }
            l.pos[0] = next[0];
            l.pos[2] = next[2];
            return;
        }
        // The surface where the feet will be after this step: a step up of
        // more than STEP_UP there is a wall. (Probing further ahead would
        // read a 45° ramp as a wall.)
        let (surface, _) = self.world.surface_below(next[0], l.pos[1] + STEP_UP + 1, next[2]);
        // Body clearance ahead: solid at step height above that surface is a
        // wall (a ramp rises at most REACH over the probe distance, which stays
        // below this), and a ceiling lower than the walker's head blocks too.
        let wall = |w: &World, y: i32| w.solid([ahead[0], y, ahead[1]]);
        let blocked = surface > l.pos[1] + STEP_UP
            || wall(&self.world, surface + STEP_UP + 2)
            || wall(&self.world, surface + HEAD_HEIGHT - 1);
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
            let ground_flags = if on_block { self.world.flags_at([l.pos[0], ground - 1, l.pos[2]]) } else { None };
            let liquid = ground_flags.is_some_and(|f| f & flags::LIQUID != 0);
            if liquid || (!on_block && self.world.floor(l.pos[0], l.pos[2]) == Floor::Water) {
                l.set_state(State::Drowning);
            } else if !self.bounce(l, 0) {
                // Slippery tops make walkers slide; leaving them, they walk.
                let slippery = ground_flags.is_some_and(|f| f & flags::SLIPPERY != 0);
                l.state = if slippery { State::Sliding } else { State::Walking };
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
