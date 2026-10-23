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
/// Falling: the speed grows by [`FALL_ACCEL`] sub-units per tick each tick up
/// to [`FALL_SPEED`] (¼ unit per tick); the lemming drifts on at walking
/// speed for the first [`FALL_DRIFT_TICKS`] ticks (measured tick by tick on
/// Practice "Trampoline", four identical falls).
const FALL_ACCEL: i32 = 8;
const FALL_SPEED: i32 = SUB / 4;
const FALL_DRIFT_TICKS: u32 = 12;
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
/// Falls of this height or more splat (owner's observations: 3¾ units
/// survived and 4 killed out of a miner's tunnel in Practice "Miner", and
/// lemmings dropping 4 units into a hole dug on Fun 4's castle die).
const SPLAT_HEIGHT: i32 = 4 * SUB;
/// Shorter falls of this height or more leave the lemming on its back for
/// [`STUN_TICKS`] (owner's observation: falls of 3, 3¼ and 3¾ units from a
/// miner's tunnel in Practice "Miner" stun, 2¾ does not; the time is ours).
const STUN_HEIGHT: i32 = 3 * SUB;
pub const STUN_TICKS: u32 = 28;
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
/// The last ticks of a fuse, after the countdown: the bomber holds its nose
/// and swells (observed: about 10 ticks after the last countdown digit).
pub const SWELL_TICKS: u32 = 10;
/// Ticks the entrance hatches take to open at the start of a level; the
/// first lemming drops on the tick after (the owner saw the hatch start
/// closed, open, and only then release; the duration is ours).
pub const HATCH_OPEN_TICKS: u32 = 10;
/// Explosion radius across the ground in sub-units (rough: the hole is
/// about one cell long).
const BLAST_RADIUS: i32 = SUB;
/// How far below and above the bomber's feet the blast reaches.
const BLAST_BELOW: i32 = SUB / 2;
const BLAST_ABOVE: i32 = SUB;
/// Ticks per segment dug (measured: 3.0 ± 0.1 s per quarter unit through
/// Practice "Digger"'s bridge, seen side-on, with the first quarter gone
/// one period after the assignment).
pub const DIG_TICKS: u32 = 42;
/// Ticks per brick laid by a builder (verified: 25 ± 1).
pub const BUILD_TICKS: u32 = 25;
/// Ticks from the assignment to the first brick: the builder stands still
/// with its sack until then (measured: 8 in the Practice "Builder" demo).
pub const FIRST_BRICK_TICKS: u32 = 8;
/// Bricks per builder (verified: 6).
const BRICKS: u8 = 6;
/// Ticks a builder takes to walk the length of a brick after laying it, at
/// walking speed.
pub const BRICK_WALK_TICKS: i32 = (BRICK_RUN + WALK_SPEED - 1) / WALK_SPEED;
/// How long a builder out of bricks shrugs (cells 474–488, three frames;
/// two ticks each, unmeasured).
const SHRUG_TICKS: u32 = 6;
/// Each brick moves the builder this far forward, 0.55 unit, and ¼ unit up
/// (measured: 37 px a brick at 67 px a unit in Practice "Builder"; a run of
/// ½ left the published bridges of Mayhem 1 and Taxing 5 about 0.3 unit
/// short); bricks reach from [`BRICK_START`] to [`BRICK_END`] ahead of the
/// feet, about 0.55 unit long (measured), so consecutive bricks overlap.
const BRICK_RUN: i32 = 141;
/// Bricks start at the builder's feet, so one laid at a block's edge leaves
/// no gap for walkers to drop through (the published routes of Taxing 19
/// "Lemmtris" and Tricky 14 "Chocolate Drop" have followers cross that way).
const BRICK_START: i32 = 0;
const BRICK_END: i32 = SUB / 2 + SUB / 8 + SUB / 16;
/// Ticks per stroke of a basher (verified: the segments go 32 ticks after the
/// first crack).
pub const BASH_TICKS: u32 = 32;
/// Ticks from a basher reaching its block to the first crack (measured: about
/// 7 in the Practice "Basher" demo); its strokes then end every
/// [`BASH_TICKS`].
pub const BASH_CRACK_START: u32 = 7;
/// Ticks per swing of a basher's mallets: one cycle of its 8-frame
/// animation (provisional).
const BASH_SWING_TICKS: u32 = 8;
/// Ticks a miner cracks the block ahead before it breaks; with the walk
/// across the broken block (a unit at walking speed, 32 ticks) a stroke
/// takes 49 ticks (rough: one cell about every 3.5 s).
pub const MINE_CRACK_TICKS: u32 = 17;
/// Cells of the block a miner breaks, each with its segments being broken.
pub type MineCells = Vec<([i32; 3], u8)>;
/// How far each block a miner breaks lies below the last (owner's
/// side-by-side comparison with the original in Practice "Miner": half a
/// unit cut far too deep a trench; ours, a segment).
const MINE_DROP: i32 = SUB / 4;
/// How far before the block ahead a miner stands to mine it: about half
/// its width, so that it stays in front of the block (owner's observation;
/// the Practice demo had the miner walk on to the edge of its cell, 14 ticks
/// from mid-cell, which this supersedes).
const MINE_STANDOFF: i32 = SUB / 4;
/// Duration of terminal animations, in ticks (provisional).
const EXIT_TICKS: u32 = 11;
pub const DEATH_TICKS: u32 = 14;
/// How long a killing trap stays busy after taking a lemming (provisional).
pub const TRAP_BUSY_TICKS: u32 = 28;
/// How long a spring shows its launch after throwing a lemming (unmeasured:
/// one tick per frame of its seven-frame animation).
pub const SPRING_BUSY_TICKS: u32 = 7;
/// Ticks a spring's sender rests after a throw, per unit thrown: a walker
/// reaching it meanwhile walks over it. Practice "Catapults" (an 18-unit
/// throw) rests between 46 and 49 ticks (owner: walkers released 46 ticks
/// apart walk over it every other time, 49 apart are all thrown); Mayhem 15
/// "Hole in One, Two, Three!" (7 units) between 17 and 25 (observed: walkers
/// 31 ticks apart all thrown, 17 apart every other one, and one throw 25
/// ticks after the last). Proportional to the distance is ours.
const SPRING_REST_PER_UNIT: f32 = 2.65;
/// How long a trampoline pad shows its dip after a lemming touches it
/// (its four frames, one per tick; timing unmeasured).
pub const PAD_BUSY_TICKS: u32 = 4;
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
/// Trampoline hops, measured tick by tick in the original's Practice
/// "Trampoline" and Fun 5 "Bounce Bounce" (`docs/spec/behaviour.md`): forward
/// speed in the air, sub-units per tick (none on the tick a pad is touched).
/// A hop of `N` ticks from pad to pad rises `120·k·(N − k)/(N + 1)` sub-units
/// above its take-off height `k` ticks in; [`MAX_HOP_TICKS`] bounds a chain.
const BOUNCE_VX: i32 = 32;
const MAX_HOP_TICKS: u32 = 486;

/// The hop off a pad reached on foot along a level path, in ticks.
const MIN_HOP_TICKS: u32 = 10;

/// Ticks a red pad adds to the hop that lands on it, and a blue pad takes off.
const HOP_STEP_TICKS: u32 = 8;

/// Height of a trampoline hop of `ticks` above its take-off, `k` ticks in
/// (negative past its end, continuing the same curve down).
fn hop_height(ticks: u32, k: i32) -> i32 {
    let t = ticks as i64;
    (120 * k as i64 * (t - k as i64) / (t + 1)) as i32
}

/// Ticks of the hop off a red pad after dropping `fallen` sub-units onto it:
/// [`MIN_HOP_TICKS`] and [`HOP_STEP_TICKS`] more per unit dropped, to the
/// nearest tick. Measured on Fun 5 "Bounce Bounce": about 10, 14 and 18
/// ticks after drops of 0, ½ and 1 unit; Practice "Trampoline": 14 after its
/// ½-unit drop. Fits the published solutions of Fun 1 "Take a Dive" (a
/// 10-unit drop must carry lemmings about 11–12 units on: 90 ticks) and
/// Tricky 2 "Which Exit?".
fn hop_after_drop(fallen: i32) -> u32 {
    let step = SUB / HOP_STEP_TICKS as i32;
    (MIN_HOP_TICKS + ((fallen.max(0) + step / 2) / step) as u32).min(MAX_HOP_TICKS)
}

/// Ticks of the hop off a blue pad after dropping `fallen` sub-units onto it:
/// 7 per unit dropped, to the nearest tick, less 4 (none below
/// [`MIN_HOP_TICKS`]: the lemming walks on). Fitted to the original: on
/// Fun 5 "Bounce Bounce" drops of 3, 2 and 1 unit give hops of about 16
/// ticks (landing before the middle of the cell beyond the water), about 10
/// ticks, and no bounce; on a test level (`docs/spec/behaviour.md`), drops
/// of 3 and 4¼ units splash down about 3 and 3.5 units on (17 and 26–27
/// ticks).
fn blue_hop_after_drop(fallen: i32) -> Option<u32> {
    let ticks = (7 * fallen.max(0) + SUB / 2) / SUB - 4;
    (ticks >= MIN_HOP_TICKS as i32).then(|| (ticks as u32).min(MAX_HOP_TICKS))
}

/// Speed on slippery blocks, sub-units per tick.
const ICE_SPEED: i32 = WALK_SPEED * 3;

/// The 22.5° ramp's shape (`docs/spec/level.md`).
const SHALLOW_RAMP: u8 = 8;

/// The 45° deflector wall's shape (`docs/spec/level.md`).
const DEFLECTOR: u8 = 7;

/// Block ids with hard-coded meaning (`docs/spec/blk.md`).
const ENTRANCE_ID: u8 = 0;
const EXIT_ID: u8 = 1;
const SPLITTER_ID: u8 = 2;

/// The height in quarter units that one dig cuts block `b` (dug down to
/// `cut` so far) down to, under a digger whose feet are `feet` above the
/// cell's bottom: a quarter off the top; a shaped block whose top reaches
/// above the feet is first cut level with them, so the digger stands on the
/// cut (a sixteenth of slack keeps a digger on a quarter line from cutting
/// below it).
fn dig_cut(b: l3d_formats::level::BlockCell, cut: u8, feet: i32) -> u8 {
    let top = (8 - b.segments.leading_zeros() as u8).min(cut);
    let level = ((feet + SUB / 16) / (SUB / 4)).clamp(0, 4) as u8;
    if b.shape != 0 && level < top {
        level
    } else {
        top.saturating_sub(1)
    }
}

/// The segments of `segments` from height `from` up to `to`, in quarter
/// units.
fn segments_between(segments: u8, from: u8, to: u8) -> u8 {
    segments & !((1u8 << from) - 1) & ((1u8 << to) - 1)
}

/// The middle of the cell containing sub-unit coordinate `v`, along one axis.
fn mid(v: i32) -> i32 {
    v.div_euclid(SUB) * SUB + SUB / 2
}

/// Whether a step from `from` to `to` along an axis heading `step` (±1)
/// reaches `centre`, having started short of it.
fn crosses(from: i32, to: i32, centre: i32, step: i32) -> bool {
    (centre - from) * step > 0 && (centre - to) * step <= 0
}

/// The point `by` sub-units on from `p` along `d`, at the same height.
fn ahead_of(p: [i32; 3], d: [i32; 3], by: i32) -> [i32; 3] {
    [p[0] + d[0] * by, p[1], p[2] + d[2] * by]
}

/// The side an exit block's doorway faces: its +Z face, turned with the
/// block's rotation (verified for all four rotations).
fn door_of(b: l3d_formats::level::BlockCell) -> Dir {
    let mut door = Dir::PosZ;
    for _ in 0..b.rotation {
        door = door.anticlockwise();
    }
    door
}

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

    /// The heading after meeting a deflector face with outward normal `n`
    /// (x, z), as if reflected.
    fn deflected(self, n: [i32; 2]) -> Dir {
        let d = self.delta();
        let k = d[0] * n[0] + d[2] * n[1];
        match [d[0] - k * n[0], d[2] - k * n[1]] {
            [1, 0] => Dir::PosX,
            [-1, 0] => Dir::NegX,
            [0, 1] => Dir::PosZ,
            _ => Dir::NegZ,
        }
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
    Falling {
        from_y: i32,
    },
    /// Falling slowly under an umbrella.
    Floating,
    Walking,
    Climbing,
    Blocking,
    /// Standing and pointing one arm sideways; walkers that reach it leave in
    /// the direction it points.
    Turning {
        to: Dir,
    },
    Digging,
    /// Laying steps; the count is the bricks still to lay.
    Building {
        bricks_left: u8,
    },
    /// A builder out of bricks, shrugging before it walks on.
    Shrugging,
    /// Removing terrain straight ahead.
    Bashing,
    /// Removing terrain ahead, a block at a time, each half a unit lower.
    Mining,
    Exiting,
    Exploding,
    Splatting,
    /// Lying on its back for a moment after a long fall, then walking on.
    Stunned,
    Drowning,
    /// Killed by the level boundary (the original shows a lightning zap).
    Zapped,
    /// Caught by a trap; the trap's own animation shows the death.
    Trapped,
    /// Hanging from a rope slide, riding towards its far end (feet
    /// position, sub-units).
    OnRope {
        to: [i32; 3],
    },
    /// Walking on a slippery block: faster, and no skill that needs a
    /// standing lemming can be given (the original shows "Sliding").
    Sliding,
    /// Bouncing on trampolines: `k` ticks into a hop lasting `ticks` from pad
    /// to pad (see [`hop_height`]), taken off at height `y0`; before tick 1 it
    /// stands on the pad (`k` below 0: ticks still to wait there).
    Bouncing {
        ticks: u32,
        k: i32,
        y0: i32,
    },
    /// Thrown through the air by a spring from `from` to
    /// `to` (feet positions) over `ticks` ticks, rising `peak`
    /// sub-units above the straight line at mid-flight.
    Flying {
        from: [i32; 3],
        to: [i32; 3],
        ticks: u32,
        peak: i32,
    },
    /// Standing on a teleporter pad, vanishing, away, and reappearing on
    /// the partner pad `to` (feet position); see [`TELEPORT_TICKS`].
    Teleporting {
        to: [i32; 3],
    },
}

/// Teleporting, in ticks from stopping on the pad centre (observed in the
/// original's Practice "Teleporter" demo, about 2 s in all): stands, is
/// squeezed to nothing, is away while sparkles show at both pads, stretches
/// out again on the partner, stands, and walks on.
pub const TELEPORT_STAND: u32 = 4;
pub const TELEPORT_VANISH: u32 = 3;
pub const TELEPORT_AWAY: u32 = 12;
pub const TELEPORT_APPEAR: u32 = 4;
pub const TELEPORT_TICKS: u32 =
    2 * TELEPORT_STAND + TELEPORT_VANISH + TELEPORT_AWAY + TELEPORT_APPEAR;

impl State {
    /// Whether the lemming is finished and only playing a last animation.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            State::Exiting
                | State::Exploding
                | State::Splatting
                | State::Drowning
                | State::Zapped
                | State::Trapped
        )
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
    /// The spring sender (index into the objects) it is walking over, which
    /// was resting when it stepped on, and the way it is crossing; that
    /// sender doesn't throw it on this crossing.
    pub passing: Option<(usize, Dir)>,
    /// Has come down from a fall and not yet taken a walking step.
    pub landed: bool,
    /// A skill given but not started yet: blockers, turners, bashers and
    /// miners walk on until they cross into the next cell along their
    /// heading, then take it up (measured in the original's Practice demos).
    pub pending: Option<State>,
    /// Ticks a basher has walked on towards its next wall since it last
    /// bashed; 0 while it bashes.
    pub bash_walk_ticks: u32,
}

impl Lemming {
    /// A lemming just released at `pos`, falling, heading `dir`.
    fn new(pos: [i32; 3], dir: Dir) -> Self {
        Lemming {
            pos,
            dir,
            state: State::Falling { from_y: pos[1] },
            state_ticks: 0,
            climber: false,
            floater: false,
            fuse: None,
            gone: false,
            teleported: false,
            passing: None,
            landed: false,
            pending: None,
            bash_walk_ticks: 0,
        }
    }

    fn set_state(&mut self, s: State) {
        self.state = s;
        self.state_ticks = 0;
        self.bash_walk_ticks = 0;
    }

    /// Starts falling from where it is.
    fn start_falling(&mut self) {
        self.set_state(State::Falling {
            from_y: self.pos[1],
        });
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

/// Something the player did to a running level. With the tick it happened
/// on (after that many steps), a list of these replays a game exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Assign { lemming: usize, skill: Skill },
    Turner { lemming: usize, to: Dir },
    ReleaseRate(i32),
    Nuke,
}

/// A blocker or turner: a lemming that redirects walkers.
#[derive(Debug, Clone, Copy)]
struct Obstacle {
    /// The index of the lemming standing there.
    lemming: usize,
    pos: [i32; 3],
    /// For turners: the heading walkers are sent off in.
    turn_to: Option<Dir>,
    /// The heading the blocker or turner had when it stopped.
    heading: Dir,
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
    /// Splitter cells, and for each heading in (in [`Dir`] order) whether
    /// the next walker through turns left.
    splitters: Vec<([i32; 3], [bool; 4])>,
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
                let spawn = [
                    x as i32 * SUB + SUB / 2,
                    y as i32 * SUB + SUB / 2 - 1,
                    z as i32 * SUB + SUB / 2,
                ];
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
            time_left: (level.time_minutes as u32 * 60 + level.time_seconds as u32)
                * TICKS_PER_SECOND,
            nuked: false,
            tick: 0,
            log: Vec::new(),
            release_timer: HATCH_OPEN_TICKS,
            next_entrance: 0,
            events: Vec::new(),
            object_kind: objects::level_objects(level).map(|(k, _)| k),
            objects: objects::level_objects(level)
                .map(|(_, o)| o)
                .unwrap_or_default(),
            splitters: level
                .cells()
                .filter(|(_, _, _, b, _)| b.id == SPLITTER_ID && !b.is_empty())
                .map(|(x, y, z, _, _)| ([x as i32, y as i32, z as i32], [true, true, false, false]))
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
        let Some(l) = self.lemmings.get(i) else {
            return false;
        };
        if l.gone || l.state.is_terminal() || self.skills_left[skill as usize] == 0 {
            return false;
        }
        // A lemming at one of the terrain skills takes another in its place
        // (inferred, as in the 2D games: Mayhem 1 "The Five Arches" needs a
        // digger to be switched to building [XeyeWalk]); not the one it has.
        let working = match l.state {
            State::Digging => Some(Skill::Digger),
            State::Building { .. } => Some(Skill::Builder),
            State::Bashing => Some(Skill::Basher),
            State::Mining => Some(Skill::Miner),
            _ => None,
        };
        match skill {
            Skill::Climber => !l.climber,
            Skill::Floater => !l.floater,
            Skill::Bomber => l.fuse.is_none(),
            // Not to a climber on its wall: there a digger is refused and
            // not paid for (verified by the owner on "Candyland Climber").
            Skill::Blocker | Skill::Turner => l.state == State::Walking && l.pending.is_none(),
            // A builder out of bricks, laid or shrugging, takes more
            // (provisional, as in the 2D games; Fun 10 "Castle Lemmalot"
            // otherwise leaves a few ticks for it).
            Skill::Builder
                if matches!(
                    l.state,
                    State::Shrugging | State::Building { bricks_left: 0 }
                ) =>
            {
                l.pending.is_none()
            }
            Skill::Digger | Skill::Builder | Skill::Basher | Skill::Miner => {
                l.pending.is_none()
                    && (l.state == State::Walking || working.is_some_and(|w| w != skill))
            }
        }
    }

    /// Gives `skill` to lemming `i`; returns whether it was accepted.
    pub fn assign(&mut self, i: usize, skill: Skill) -> bool {
        let ok = self.give(i, skill);
        if ok {
            self.log
                .push((self.tick, Command::Assign { lemming: i, skill }));
        }
        ok
    }

    fn give(&mut self, i: usize, skill: Skill) -> bool {
        if !self.can_assign(i, skill) {
            return false;
        }
        self.skills_left[skill as usize] -= 1;
        let l = &mut self.lemmings[i];
        // Switched from another terrain skill, or given more bricks, a
        // lemming starts where it stands (provisional).
        if l.state != State::Walking
            && let Some(s) = match skill {
                Skill::Basher => Some(State::Bashing),
                Skill::Miner => Some(State::Mining),
                Skill::Digger => Some(State::Digging),
                Skill::Builder => Some(State::Building {
                    bricks_left: BRICKS,
                }),
                _ => None,
            }
        {
            l.set_state(s);
            return true;
        }
        match skill {
            Skill::Climber => l.climber = true,
            Skill::Floater => l.floater = true,
            Skill::Bomber => l.fuse = Some(FUSE_TICKS),
            // Diggers and builders start at the middle of their cell, miners
            // a little before its edge, the others at the edge (see `update`).
            Skill::Blocker => l.pending = Some(State::Blocking),
            // The turner's own left (the one verified case; see `assign_turner`).
            Skill::Turner => {
                l.pending = Some(State::Turning {
                    to: l.dir.anticlockwise(),
                })
            }
            Skill::Digger => l.pending = Some(State::Digging),
            Skill::Builder => {
                l.pending = Some(State::Building {
                    bricks_left: BRICKS,
                })
            }
            Skill::Basher => l.pending = Some(State::Bashing),
            Skill::Miner => l.pending = Some(State::Mining),
        }
        true
    }

    /// Makes lemming `i` a turner pointing `to`, a quarter turn either way
    /// from its heading. In the original the player picks the side with a
    /// second click (`docs/spec/behaviour.md`, "A turner takes two clicks").
    pub fn assign_turner(&mut self, i: usize, to: Dir) -> bool {
        let Some(l) = self.lemmings.get(i) else {
            return false;
        };
        if to == l.dir || to == l.dir.reverse() || !self.give(i, Skill::Turner) {
            return false;
        }
        self.lemmings[i].pending = Some(State::Turning { to });
        self.log
            .push((self.tick, Command::Turner { lemming: i, to }));
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
                self.lemmings.push(Lemming::new(e.spawn, e.dir));
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
            o.rest = o.rest.saturating_sub(1);
        }
        // A turner turns walkers from the moment it is given, while it still
        // walks on to the edge of its cell (measured in the original).
        let obstacles: Vec<Obstacle> =
            self.lemmings
                .iter()
                .enumerate()
                .filter(|(_, l)| !l.gone)
                .filter_map(|(i, l)| {
                    let turn_to = match (l.state, l.pending) {
                        (State::Blocking, _) => None,
                        (State::Turning { to }, _)
                        | (State::Walking, Some(State::Turning { to })) => Some(to),
                        _ => return None,
                    };
                    Some(Obstacle {
                        lemming: i,
                        pos: l.pos,
                        turn_to,
                        heading: l.dir,
                    })
                })
                .collect();
        for i in 0..self.lemmings.len() {
            if !self.lemmings[i].gone {
                let mut l = self.lemmings[i].clone();
                self.update(&mut l, i, &obstacles);
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
        let changed =
            std::mem::discriminant(&a) != std::mem::discriminant(&b) || after.state_ticks == 0;
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
                State::Shrugging => Some(Event::BuilderDone),
                _ => None,
            };
        }
        let on = |n: u32| after.state_ticks.is_multiple_of(n);
        match a {
            State::Building { bricks_left } if matches!(b, State::Building { bricks_left: was } if was > bricks_left) => {
                Some(Event::Brick)
            }
            State::Digging if on(DIG_TICKS) => Some(Event::Dug),
            State::Bashing if on(BASH_SWING_TICKS) && self.bash_cell_full(after) => {
                Some(Event::Bashed)
            }
            State::Mining if after.state_ticks == MINE_CRACK_TICKS => Some(Event::Mined),
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
            self.log
                .push((self.tick, Command::ReleaseRate(rate - self.release_rate)));
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
        for l in self
            .lemmings
            .iter_mut()
            .filter(|l| !l.gone && !l.state.is_terminal())
        {
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

    /// The middle of the level's boundary (the area lemmings are zapped
    /// outside of), in units, as `[x, z]`.
    pub fn centre(&self) -> [f32; 2] {
        let [min_x, min_z, max_x, max_z] = self.border;
        [
            (min_x + max_x + 1) as f32 / 2.0,
            (min_z + max_z + 1) as f32 / 2.0,
        ]
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

    fn update(&mut self, l: &mut Lemming, me: usize, obstacles: &[Obstacle]) {
        l.state_ticks += 1;
        if let Some(f) = &mut l.fuse
            && !l.state.is_terminal()
        {
            *f = f.saturating_sub(1);
            if *f == 0 {
                self.explode(l);
                return;
            }
            // Holding its nose and swelling, a bomber on its feet stands
            // still (owner's observation); one in the air keeps moving.
            let on_feet = matches!(
                l.state,
                State::Walking
                    | State::Sliding
                    | State::Building { .. }
                    | State::Shrugging
                    | State::Bashing
                    | State::Mining
                    | State::Digging
                    | State::Climbing
            );
            if *f <= SWELL_TICKS && on_feet {
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
            State::Stunned => {
                if l.state_ticks >= STUN_TICKS {
                    l.set_state(State::Walking);
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
            State::Walking | State::Sliding => {
                if !(l.state == State::Walking && Self::take_up_pending(l)) {
                    let d = l.dir.delta();
                    let to_middle =
                        matches!(l.pending, Some(State::Building { .. } | State::Digging));
                    let dir = l.dir;
                    self.walk(l, me, obstacles);
                    // Or at a wall that would turn the walker round, facing
                    // the wall (the original's basher demo bashes the crate it
                    // walked up to). A miner steps up to its place before the cell's edge when
                    // the wall starts there, so that it mines the wall and not
                    // the cell it stands in (measured: the skills start at the
                    // edge of the cell; inferred: in Tricky 3 "This Is the
                    // Army" a miner on a thin plank mines through the wall
                    // ahead; bashers stay put, their stroke reaching the wall
                    // either way). A builder turns and walks back to the
                    // middle (ours).
                    if l.state == State::Walking
                        && l.dir == dir.reverse()
                        && !to_middle
                        && let Some(s) = l.pending.take()
                    {
                        l.dir = dir;
                        let at = Self::mine_stand(l.pos, d);
                        if s == State::Mining
                            && Self::to_edge(l.pos, d) > MINE_STANDOFF
                            && [SUB / 8, HEAD_HEIGHT - 1]
                                .iter()
                                .all(|&h| !self.world.solid([at[0], at[1] + h, at[2]]))
                        {
                            l.pos = at;
                        }
                        l.set_state(s);
                    }
                }
            }
            State::Bouncing { ticks, k, y0 } => self.fly(l, ticks, k, y0),
            State::Climbing => self.climb(l),
            State::Digging => self.dig(l),
            State::Building { bricks_left } => self.build(l, bricks_left),
            State::Shrugging => {
                if l.state_ticks >= SHRUG_TICKS {
                    l.set_state(State::Walking);
                }
            }
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
                let len = ((d[0] as i64).pow(2) + (d[1] as i64).pow(2) + (d[2] as i64).pow(2))
                    .isqrt() as i32;
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
            State::Flying {
                from,
                to,
                ticks,
                peak,
            } => {
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
                    l.start_falling();
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

    /// Starts a walker's pending skill if it has reached the place to: the
    /// edge of its cell, where the next step would cross it, standing on the
    /// edge (before any fall beyond it); a digger at the middle of the cell,
    /// where the next step would reach or pass it (owner's observation: it
    /// starts in the middle of a block); a builder at the middle or the
    /// edge, whichever comes first ([LOAP]; the owner saw it walk on with
    /// the sack before building); a miner [`MINE_STANDOFF`] before the edge,
    /// in front of the block ahead (owner's observation). Returns whether
    /// it started.
    fn take_up_pending(l: &mut Lemming) -> bool {
        let Some(s) = l.pending else {
            return false;
        };
        let d = l.dir.delta();
        let to_edge = Self::to_edge(l.pos, d);
        let at_mid = |v: i32, step: i32| {
            step == 0 || ((v - mid(v)) * step <= 0 && (v + step * WALK_SPEED - mid(v)) * step >= 0)
        };
        if matches!(s, State::Building { .. } | State::Digging)
            && at_mid(l.pos[0], d[0])
            && at_mid(l.pos[2], d[2])
        {
            for a in [0, 2] {
                if d[a] != 0 {
                    l.pos[a] = mid(l.pos[a]);
                }
            }
        } else if s == State::Mining {
            if to_edge - WALK_SPEED > MINE_STANDOFF {
                return false;
            }
            if to_edge > MINE_STANDOFF {
                l.pos = Self::mine_stand(l.pos, d);
            }
        } else if to_edge <= WALK_SPEED && s != State::Digging {
            l.pos = ahead_of(l.pos, d, to_edge - 1);
        } else {
            return false;
        }
        l.pending = None;
        l.set_state(s);
        true
    }

    /// Killing traps take a lemming that reaches the middle of their cell,
    /// then are busy for a while, letting others pass (provisional, as in
    /// the 2D games). Taking it anywhere in the cell would let none pass on
    /// foot, a cell taking longer to cross than a trap is busy, while Taxing
    /// 5 "3D - A Lemming Odyssey" lines its exit with traps.
    fn spring_traps(&mut self, l: &mut Lemming) {
        if !self.object_kind.is_some_and(|k| k.kills())
            || l.state.is_terminal()
            || matches!(l.state, State::Falling { .. } | State::Floating)
        {
            return;
        }
        let d = l.dir.delta();
        let off = |v: i32| (v.rem_euclid(SUB) - SUB / 2).abs();
        if off(l.pos[0]) * d[0].abs() + off(l.pos[2]) * d[2].abs() > SUB / 8 {
            return;
        }
        if let Some(o) = self
            .objects
            .iter_mut()
            .find(|o| o.busy == 0 && o.touches(l.pos))
        {
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
        if self.object_kind != Some(objects::ObjectKind::Teleporter)
            || !matches!(l.state, State::Walking | State::Sliding)
        {
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
        let pair: Vec<usize> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.value == value)
            .map(|(i, _)| i)
            .collect();
        let [.., a, b] = pair[..] else { return };
        let to = match here {
            h if h == a => b,
            h if h == b => a,
            _ => return,
        };
        // Wait for the centre of the pad.
        let centre = self.objects[here].feet();
        let d = l.dir.delta();
        if (centre[0] - l.pos[0]) * d[0] + (centre[2] - l.pos[2]) * d[2] > 0 {
            return;
        }
        l.pos[0] = centre[0];
        l.pos[2] = centre[2];
        l.set_state(State::Teleporting {
            to: self.objects[to].feet(),
        });
    }

    /// For a sender (an even value) the lemming at `p` stands on: the
    /// sender and its receiver, the next value up. With several of a value
    /// only the last works [L3DEdit].
    fn sender_at(&self, p: [i32; 3]) -> Option<(usize, usize)> {
        let last = |v: u8| self.objects.iter().rposition(|o| o.value == v);
        let sender = self
            .objects
            .iter()
            .position(|o| o.value % 2 == 0 && o.touches(p))?;
        let value = self.objects[sender].value;
        (last(value) == Some(sender)).then_some(())?;
        Some((sender, last(value + 1)?))
    }

    /// Rope slides: a lemming reaching a sender rides to its receiver
    /// ([`Self::sender_at`]). One lemming rides a rope at a time; others reaching
    /// the sender meanwhile walk on (owner's observation: at release rate
    /// 11 every other lemming of Practice "Rope Slide" is lost). Provisional:
    /// a straight line at [`ROPE_SPEED`], then it walks on in its old
    /// direction.
    fn board_rope_slide(&mut self, l: &mut Lemming) {
        if self.object_kind != Some(objects::ObjectKind::RopeSlide)
            || !matches!(
                l.state,
                State::Walking | State::Sliding | State::Falling { .. }
            )
        {
            return;
        }
        let Some((_, receiver)) = self.sender_at(l.pos) else {
            return;
        };
        let to = self.objects[receiver].feet();
        if self
            .lemmings
            .iter()
            .any(|r| !r.gone && r.state == State::OnRope { to })
        {
            return;
        }
        l.set_state(State::OnRope { to });
    }

    /// Springs: a walker stepping onto a sender is thrown in an arc onto its
    /// receiver ([`Self::sender_at`]) and lands unhurt, walking on in its old direction.
    /// Flight time and arc height are unmeasured. A sender still resting
    /// from a throw ([`SPRING_REST_PER_UNIT`]) lets a walker walk over it; the
    /// sender's rest then ends as usual, and it throws the next walker to
    /// step on, or one that turned round on it (a crossing ends there).
    fn spring(&mut self, l: &mut Lemming) {
        if self.object_kind != Some(objects::ObjectKind::Spring)
            || !matches!(l.state, State::Walking | State::Sliding)
        {
            return;
        }
        let pair = self.sender_at(l.pos);
        if l.passing.map(|(s, _)| s) != pair.map(|(s, _)| s)
            || l.passing.is_some_and(|(_, d)| d != l.dir)
        {
            l.passing = None;
        }
        let Some((sender, receiver)) = pair else {
            return;
        };
        if self.objects[sender].rest > 0 {
            l.passing = Some((sender, l.dir));
        }
        if l.passing.is_some() {
            return;
        }
        let to = self.objects[receiver].feet();
        let span = ((to[0] - l.pos[0]) as f32).hypot((to[2] - l.pos[2]) as f32) / SUB as f32;
        let ticks = ((span * SPRING_TICKS_PER_UNIT) as u32).max(8);
        let peak = (span * SPRING_ARC * SUB as f32) as i32;
        self.objects[sender].busy = SPRING_BUSY_TICKS;
        self.objects[sender].rest = (span * SPRING_REST_PER_UNIT) as u32;
        l.set_state(State::Flying {
            from: l.pos,
            to,
            ticks,
            peak,
        });
    }

    /// Starts the dip of the trampoline pad under `p` (played only on
    /// contact, as in the original).
    fn press_pad(&mut self, p: [i32; 3]) {
        if let Some(o) = self.objects.iter_mut().find(|o| o.touches(p)) {
            o.busy = PAD_BUSY_TICKS;
        }
    }

    /// Trampolines: a lemming arriving on a pad bounces into the hop
    /// [`hop_after_drop`] gives for the height it dropped off a red pad, or
    /// [`blue_hop_after_drop`] off a blue one (which may let it walk on).
    /// Stepping down at most half a unit onto it, it stands one more tick on
    /// the pad (measured: a walker
    /// stepping down onto the first pad of Practice "Trampoline" stands there
    /// two ticks); from a longer drop it bounces straight off as from a hop
    /// (observed in the lemming view on Tricky 2 "Which Exit?": after its
    /// 1¾-unit drop the view rests on the pad one tick). A lemming walking
    /// onto a pad level with its path walks to the pad's middle first
    /// (measured on Fun 5 "Bounce Bounce"; lemmings stepping or dropping
    /// onto a pad bounce where they touch it). Returns whether it bounced.
    fn bounce(&mut self, l: &mut Lemming, height: i32) -> bool {
        if self.object_kind != Some(objects::ObjectKind::Trampoline) {
            return false;
        }
        let Some(pad) = self.objects.iter().find(|o| o.touches(l.pos)) else {
            return false;
        };
        let Some(ticks) = (if pad.value >= 0x64 {
            blue_hop_after_drop(height)
        } else {
            Some(hop_after_drop(height))
        }) else {
            return false;
        };
        if height == 0 && matches!(l.state, State::Walking | State::Sliding) {
            let d = l.dir.delta();
            let into = |v: i32, step: i32| match step {
                1 => v.rem_euclid(SUB),
                -1 => SUB - 1 - v.rem_euclid(SUB),
                _ => SUB,
            };
            if into(l.pos[0], d[0]).min(into(l.pos[2], d[2])) < SUB / 2 {
                return false;
            }
        }
        self.press_pad(l.pos);
        l.set_state(State::Bouncing {
            ticks,
            k: if height > SUB / 2 { 0 } else { -1 },
            y0: l.pos[1],
        });
        true
    }

    /// One tick of a bouncing lemming: forward [`BOUNCE_VX`] and along its
    /// hop's height curve. Walls turn it back and deflectors turn it aside in
    /// the air; coming down onto a pad it stands there for the tick and
    /// bounces into the next hop up; elsewhere it lands unhurt.
    fn fly(&mut self, l: &mut Lemming, ticks: u32, k: i32, y0: i32) {
        if k < 0 {
            // Still on the pad, stepping on at walking speed (measured: the
            // first pad's take-off is about 8 sub-units past the touch).
            let d = l.dir.delta();
            let ahead = ahead_of(l.pos, d, WALK_SPEED);
            if !self.world.solid([ahead[0], ahead[1] + SUB / 8, ahead[2]]) {
                l.pos = ahead;
            }
            l.state = State::Bouncing {
                ticks,
                k: k + 1,
                y0,
            };
            return;
        }
        let d = l.dir.delta();
        let k1 = k + 1;
        let y = y0 + hop_height(ticks, k1);
        let rising = y >= l.pos[1];
        let next = [l.pos[0] + d[0] * BOUNCE_VX, y, l.pos[2] + d[2] * BOUNCE_VX];
        let body = next[1] + SUB / 8;
        let advance = |l: &mut Lemming| l.state = State::Bouncing { ticks, k: k1, y0 };
        if let Some(n) = self
            .deflector_across(next[0], next[1], next[2])
            .filter(|n| d[0] * n[0] + d[2] * n[1] < 0)
        {
            l.dir = l.dir.deflected(n);
            // Turned at the centre of the deflector's cell (inferred: lemmings
            // turned in the air at the far corner of Practice "Trampoline"
            // must land on the one-cell-wide back row).
            l.pos[0] = mid(next[0]);
            l.pos[2] = mid(next[2]);
            l.pos[1] = y;
            advance(l);
            return;
        }
        // A hop that carries the lemming into an exit through its doorway
        // saves it (inferred: Tricky 2 "Which Exit?" is solved by bouncing
        // into a door raised above the pads).
        if let Some(b) = self
            .world
            .block_at([next[0], next[1] + SUB / 8, next[2]])
            .filter(|b| b.id == EXIT_ID)
            && l.dir == door_of(b).reverse()
        {
            l.pos = next;
            l.set_state(State::Exiting);
            return;
        }
        let wall =
            |w: &World, y: i32| w.solid([next[0], y, next[2]]) && !w.solid([l.pos[0], y, l.pos[2]]);
        if rising && (wall(&self.world, body) || wall(&self.world, next[1] + HEAD_HEIGHT - 1)) {
            l.dir = l.dir.reverse();
            l.pos[1] = y;
            advance(l);
            return;
        }
        if !rising {
            let (ground, on_block) = self
                .world
                .surface_below(next[0], l.pos[1] + STEP_UP, next[2]);
            // Terrain at body height ahead (a slope's side) turns it back.
            if self.world.solid([next[0], ground + STEP_UP + 1, next[2]])
                || ground > l.pos[1] + STEP_UP
            {
                l.dir = l.dir.reverse();
                l.pos[1] = y.max(ground);
                advance(l);
                return;
            }
            // The touching tick, where the hop comes down to the surface
            // under it or just ahead: down onto it without moving on, so the
            // surface it stays over counts (measured: no forward move on the
            // touching tick; inferred: in Tricky 2 "Which Exit?" a hop that
            // comes down at the far edge of a pad must bounce off it to reach
            // the exit door).
            let (here, here_on_block) =
                self.world
                    .surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
            let (ground, on_block) = if here >= next[1] {
                (here, here_on_block)
            } else {
                (ground, on_block)
            };
            if ground >= next[1] {
                l.pos[1] = ground;
                let pad = (self.object_kind == Some(objects::ObjectKind::Trampoline))
                    .then(|| self.objects.iter().find(|o| o.touches(l.pos)))
                    .flatten()
                    .copied();
                if let Some(pad) = pad {
                    self.press_pad(l.pos);
                    // Red pads (0x60–0x63) throw the next hop
                    // [`HOP_STEP_TICKS`] longer, blue ones (0x64–0x67) that
                    // much shorter, and off a blue pad after the shortest hop
                    // the lemming walks on (measured: hops of 14, 22, 30 ticks
                    // up the red row of Practice "Trampoline"; inferred: its
                    // demo's lemmings then come down along blue ones to walk
                    // into the exit). A pad lower than the take-off adds its
                    // drop as [`hop_after_drop`] does (inferred).
                    let step = (SUB / HOP_STEP_TICKS as i32) as i64;
                    let level = ticks as i64 + (y0 - ground) as i64 / step;
                    let next = if pad.value >= 0x64 {
                        level - HOP_STEP_TICKS as i64
                    } else {
                        level + HOP_STEP_TICKS as i64
                    };
                    if next >= MIN_HOP_TICKS as i64 {
                        l.set_state(State::Bouncing {
                            ticks: (next as u32).min(MAX_HOP_TICKS),
                            k: 0,
                            y0: ground,
                        });
                    } else {
                        l.set_state(State::Walking);
                    }
                } else {
                    self.land(l, on_block, 0);
                }
                return;
            }
            if wall(&self.world, body) {
                l.dir = l.dir.reverse();
                l.pos[1] = y;
                advance(l);
                return;
            }
        } else if self.world.solid([next[0], next[1] + HEAD_HEIGHT, next[2]]) {
            // A ceiling stops the rise: the hop goes on from its way down.
            let mirrored = ticks as i32 - k;
            l.state = State::Bouncing {
                ticks,
                k: mirrored.max(k1),
                y0,
            };
            return;
        }
        l.pos = next;
        advance(l);
    }

    /// Ends a bomber by blowing a hole: every column whose centre lies within
    /// the blast radius of the bomber (across the ground) loses the same band, from half a
    /// unit below its feet to a unit above them, so a wall it stands at goes
    /// whole and the crater floor is level (ours; a sphere left floating
    /// halves of walls and uneven floors that trapped walkers).
    fn explode(&mut self, l: &mut Lemming) {
        let r = BLAST_RADIUS;
        let (lo, hi) = (l.pos[1] - BLAST_BELOW, l.pos[1] + BLAST_ABOVE);
        let cell = |v: i32| v.div_euclid(SUB);
        for cx in cell(l.pos[0] - r)..=cell(l.pos[0] + r) {
            for cz in cell(l.pos[2] - r)..=cell(l.pos[2] + r) {
                let d = [cx * SUB + SUB / 2 - l.pos[0], cz * SUB + SUB / 2 - l.pos[2]]
                    .map(|v| v as i64);
                if d[0] * d[0] + d[1] * d[1] > (r as i64) * (r as i64) {
                    continue;
                }
                for cy in cell(lo)..=cell(hi - 1) {
                    // Segments whose centres lie in the band.
                    let mask = (0..4)
                        .filter(|seg| (lo..hi).contains(&(cy * SUB + seg * SUB / 4 + SUB / 8)))
                        .fold(0u8, |m, seg| m | 1 << seg);
                    let c = [cx, cy, cz];
                    let low = mask.trailing_zeros();
                    let above_band =
                        self.world.present(c) & !mask & 0xFu8.checked_shl(low).unwrap_or(0);
                    // A shaped block losing its top is shaved flat and keeps
                    // its slopes below the cut, as for the digger (blasts on
                    // a hill of 45° ramps in the original left flat steps
                    // with vertical faces, no shallower slope).
                    match self.world.block(c) {
                        Some((b, _))
                            if b.shape != 0
                                && mask != 0
                                && above_band == 0
                                && self.world.breakable_towards(c, None) =>
                        {
                            self.world.shave(c, low as u8);
                        }
                        _ => {
                            self.world.remove_segments(c, mask);
                        }
                    }
                }
            }
        }
        l.fuse = None;
        l.set_state(State::Exploding);
    }

    /// For feet at `p`, on a block or on the level floor: the flags of the
    /// block stood on, and whether that is water.
    fn underfoot(&self, p: [i32; 3], on_block: bool) -> (Option<u8>, bool) {
        let f = on_block
            .then(|| self.world.flags_at([p[0], p[1] - 1, p[2]]))
            .flatten();
        let wet = f.is_some_and(|f| f & flags::LIQUID != 0)
            || (!on_block && self.world.floor(p[0], p[2]) == Floor::Water);
        (f, wet)
    }

    /// What becomes of a lemming whose feet are in liquid, the sea or a liquid
    /// block top: it drowns, unless the level makes liquid electric.
    fn sunk(&self) -> State {
        if self.world.liquid_zaps {
            State::Zapped
        } else {
            State::Drowning
        }
    }

    /// Lands a lemming on whatever is at its feet.
    fn land(&mut self, l: &mut Lemming, on_block: bool, fell: i32) {
        if self.bounce(l, fell) {
            return;
        }
        l.landed = true;
        let (ground_flags, wet) = self.underfoot(l.pos, on_block);
        if wet {
            l.set_state(self.sunk());
        } else if ground_flags.is_some_and(|f| f & flags::NO_SPLAT != 0) {
            l.set_state(State::Walking);
        } else if fell >= SPLAT_HEIGHT {
            l.set_state(State::Splatting);
        } else if fell >= STUN_HEIGHT {
            l.set_state(State::Stunned);
        } else {
            l.set_state(State::Walking);
        }
    }

    fn fall(&mut self, l: &mut Lemming, from_y: i32) {
        // Drifting forward at walking speed for the first ticks, unless
        // something solid is in the way.
        let d = if l.state_ticks <= FALL_DRIFT_TICKS {
            l.dir.delta()
        } else {
            [0; 3]
        };
        let ahead = ahead_of(l.pos, d, WALK_SPEED);
        if !self.world.solid([ahead[0], ahead[1] + SUB / 8, ahead[2]])
            && !self
                .world
                .solid([ahead[0], ahead[1] + HEAD_HEIGHT - 1, ahead[2]])
        {
            l.pos[0] = ahead[0];
            l.pos[2] = ahead[2];
        }
        let speed = (FALL_ACCEL * l.state_ticks.max(1) as i32).min(FALL_SPEED);
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
        if self
            .world
            .terrain_solid([l.pos[0], l.pos[1] + HEAD_HEIGHT + CLIMB_SPEED, l.pos[2]])
        {
            // Ceiling: let go and fall back. Bricks are no ceiling: in the
            // original a climber climbed a wall straight past the first
            // brick of a builder standing at its top edge.
            l.dir = l.dir.reverse();
            l.start_falling();
            return;
        }
        l.pos[1] += CLIMB_SPEED;
        if !self.world.solid([ahead[0], l.pos[1], ahead[1]])
            && !self
                .world
                .solid([ahead[0], l.pos[1] + HEAD_HEIGHT - 1, ahead[1]])
        {
            // Reached the top edge, with room for the body: step onto it.
            l.pos[0] = ahead[0];
            l.pos[2] = ahead[1];
            let (ground, _) = self
                .world
                .surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
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
        let (ground, _) = self
            .world
            .surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
        if ground < l.pos[1] - STEP_DOWN {
            l.start_falling();
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
        if l.state_ticks < FIRST_BRICK_TICKS {
            return;
        }
        // After laying a brick the builder steps up onto it and walks along
        // it at walking speed to where the next is laid (owner: it walks
        // over the brick, it does not jump to its end).
        let phase = ((l.state_ticks - FIRST_BRICK_TICKS) % BUILD_TICKS) as i32;
        if (1..=BRICK_WALK_TICKS).contains(&phase) {
            let d = l.dir.delta();
            let run = |p: i32| BRICK_RUN * p / BRICK_WALK_TICKS;
            let step = run(phase) - run(phase - 1);
            l.pos[0] += d[0] * step;
            l.pos[2] += d[2] * step;
            if phase == 1 {
                l.pos[1] += SUB / 4;
            }
            return;
        }
        if phase != 0 {
            return;
        }
        if bricks_left == 0 {
            l.set_state(State::Shrugging);
            return;
        }
        let d = l.dir.delta();
        // A builder standing a hair above a segment line (on the thin edge
        // of a ramp) builds from the line, so that walkers can step onto its
        // first brick (Fun 10 "Castle Lemmalot", whose published staircase
        // starts at the foot of a ramp).
        let above = l.pos[1].rem_euclid(SUB / 4);
        if bricks_left == BRICKS && above > 0 && above < SUB / 16 {
            l.pos[1] -= above;
        }
        let template = match self
            .world
            .block(Self::cell_of([l.pos[0], l.pos[1] - 1, l.pos[2]]))
        {
            Some((b, _)) if b.id >= 9 => b.id,
            _ => self.world.common_id,
        };
        // A brick from just ahead of the feet to just past the next standing
        // point, one segment thick, a unit wide (width unmeasured).
        let along = |from: i32, to: i32, axis: usize| {
            let (a, b) = (l.pos[axis] + d[axis] * from, l.pos[axis] + d[axis] * to);
            if d[axis] == 0 {
                (l.pos[axis] - SUB / 2, l.pos[axis] + SUB / 2)
            } else {
                (a.min(b), a.max(b))
            }
        };
        let next = [
            l.pos[0] + d[0] * BRICK_RUN,
            l.pos[1] + SUB / 4,
            l.pos[2] + d[2] * BRICK_RUN,
        ];
        let head_blocked = [next[1] + 1, next[1] + HEAD_HEIGHT - 1]
            .iter()
            .any(|&y| self.world.solid([next[0], y, next[2]]));
        // A brick that would run into terrain is cut short at it, filling
        // the gap up to the wall, and is laid even where the builder's head
        // would not fit on it; the builder then stops (observed in Practice
        // "Builder" with a wall across the bridge: the brick running into it
        // is laid, then the builder walks back).
        let mut end = BRICK_END;
        let laid = loop {
            let (x0, x1) = along(BRICK_START, end, 0);
            let (z0, z1) = along(BRICK_START, end, 2);
            let brick = world::Brick {
                min: [x0, l.pos[1], z0],
                max: [x1, l.pos[1] + SUB / 4, z1],
                id: template,
            };
            if (!head_blocked || end < BRICK_END) && self.world.add_brick(brick) {
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
        }
        // It climbs onto the new brick over the next ticks.
        l.state = State::Building {
            bricks_left: bricks_left - 1,
        };
    }

    /// For a basher at work: the cell it is bashing, the segments it will
    /// remove there (the lower half-unit from its feet up), and the ticks
    /// since that stroke's cracks began (0 to [`BASH_TICKS`] − 1), once they
    /// have. `None` before the first crack or when nothing is there to bash.
    pub fn bash_target(&self, l: &Lemming) -> Option<([i32; 3], u8, u32)> {
        if l.state != State::Bashing || l.state_ticks < BASH_CRACK_START {
            return None;
        }
        let (cell, mask) = Self::bash_cell(l);
        let present = self.world.present(cell) & mask;
        (present != 0).then(|| {
            (
                cell,
                present,
                (l.state_ticks - BASH_CRACK_START) % BASH_TICKS,
            )
        })
    }

    /// The cell a basher works on and the segments it removes there.
    fn bash_cell(l: &Lemming) -> ([i32; 3], u8) {
        Self::bash_cell_at(l, SUB / 2)
    }

    /// The cell `along` sub-units ahead of a basher and the segments it
    /// would remove there.
    fn bash_cell_at(l: &Lemming, along: i32) -> ([i32; 3], u8) {
        let d = l.dir.delta();
        let ahead = ahead_of(l.pos, d, along);
        let cell = Self::cell_of(ahead);
        // Segments from the feet up to about head height.
        let first = ((l.pos[1] - cell[1] * SUB) / (SUB / 4)).clamp(0, 3);
        // The lower half-unit (two segments) from the feet up (verified, rough).
        (cell, (0b11u8 << first) & 0xF)
    }

    /// Whether anything is left in the segments a basher works on.
    fn bash_cell_full(&self, l: &Lemming) -> bool {
        let (cell, mask) = Self::bash_cell(l);
        self.world.present(cell) & mask != 0
    }

    /// Whether the block a basher works on is a 22.5° ramp, which it cuts
    /// level at its feet instead of walking up it.
    fn bash_cell_shallow_ramp(&self, l: &Lemming) -> bool {
        let (cell, _) = Self::bash_cell(l);
        self.world
            .block(cell)
            .is_some_and(|(b, _)| b.shape == SHALLOW_RAMP)
    }

    /// Whether the block a basher works on is a 45° deflector wall turning
    /// its diagonal face towards the basher (open just inside the cell's
    /// near side, where a square back would be solid).
    fn bash_cell_deflector_facing(&self, l: &Lemming) -> bool {
        let (cell, _) = Self::bash_cell(l);
        if !self
            .world
            .block(cell)
            .is_some_and(|(b, _)| b.shape == DEFLECTOR)
        {
            return false;
        }
        let d = l.dir.delta();
        let mut near = l.pos;
        near[1] += SUB / 8;
        for a in [0, 2] {
            if d[a] != 0 {
                let edge = if d[a] > 0 {
                    cell[a] * SUB
                } else {
                    (cell[a] + 1) * SUB - 1
                };
                near[a] = edge + d[a] * (SUB / 16);
            }
        }
        !self.world.solid(near)
    }

    /// Whether solid terrain is within a walker's reach ahead of a basher, at
    /// the height of the segments it bashes. A cube's side is as soon as the
    /// basher reaches its cell; a deflector's diagonal face lies further in.
    fn bash_reaches_solid(&self, l: &Lemming) -> bool {
        let d = l.dir.delta();
        [SUB / 8, 3 * SUB / 8].into_iter().any(|h| {
            (1..=REACH).step_by(WALK_SPEED as usize).any(|a| {
                self.world
                    .solid([l.pos[0] + d[0] * a, l.pos[1] + h, l.pos[2] + d[2] * a])
            })
        })
    }

    /// Whether a basher `ticks` into bashing ends a stroke now.
    fn bash_stroke_ends(ticks: u32) -> bool {
        ticks >= BASH_CRACK_START + BASH_TICKS
            && (ticks - BASH_CRACK_START).is_multiple_of(BASH_TICKS)
    }

    /// Whether there is wall a basher can break in the cell after the one it
    /// works on.
    fn bash_more_ahead(&self, l: &Lemming) -> bool {
        let (next, mask) = Self::bash_cell_at(l, SUB / 2 + SUB);
        self.world.present(next) & mask != 0
            && self.world.breakable_towards(next, Some(l.dir.delta()))
    }

    /// Removes body-height terrain in the cell ahead and moves into the gap;
    /// with more wall in the next cell it walks on through the gap to it and
    /// bashes on, so walls thicker than a cell can be bashed through (Tricky
    /// 3 "This Is the Army"); otherwise it walks on as soon as nothing is
    /// left to bash (provisional).
    fn bash(&mut self, l: &mut Lemming) {
        let d = l.dir.delta();
        let (cell, mask) = Self::bash_cell(l);
        let full = self.bash_cell_full(l);
        if full && !self.world.breakable_towards(cell, Some(d)) {
            // Steel or a one-way block facing elsewhere: no cracks, it walks
            // on (owner's observation).
            l.set_state(State::Walking);
            return;
        }
        let shallow = self.bash_cell_shallow_ramp(l);
        if full && self.bash_cell_deflector_facing(l) && !self.bash_reaches_solid(l) {
            // A deflector's diagonal face out of reach: the basher walks to
            // it as a walker and is turned there (verified on a changed "Team
            // Work", 2026-10-22: taking the skill up at the edge of the
            // deflector's cell, or meeting the face half a unit after a
            // bashed cube, the basher left the deflector whole). A square
            // back is a wall like any other.
            l.set_state(State::Walking);
            return;
        }
        if (!full && self.bash_more_ahead(l)) || (full && !shallow && !self.bash_reaches_solid(l)) {
            // On through the gap; the next stroke starts at the wall.
            l.pos[0] += d[0] * WALK_SPEED;
            l.pos[2] += d[2] * WALK_SPEED;
            l.bash_walk_ticks += 1;
            if self.settle(l) {
                l.state_ticks = 0;
            }
            return;
        }
        l.bash_walk_ticks = 0;
        if !Self::bash_stroke_ends(l.state_ticks) {
            return;
        }
        match self.world.block(cell) {
            Some(_) if self.world.present(cell) & mask != 0 => {
                // A 22.5° ramp is cut level at the feet (observed in a
                // Practice "Basher" changed to have one before the crate: a
                // notch with a vertical face, the basher standing in it);
                // a shaped block keeps its slope below the cut.
                let cut = if shallow {
                    self.world.breakable_towards(cell, Some(d))
                        && self.world.shave(cell, mask.trailing_zeros() as u8)
                } else {
                    self.world.remove_segments_towards(cell, mask, Some(d))
                };
                if !cut {
                    l.set_state(State::Walking);
                    return;
                }
                l.pos[0] += d[0] * (SUB / 4);
                l.pos[2] += d[2] * (SUB / 4);
                if self.settle(l) && !self.bash_cell_full(l) && !self.bash_more_ahead(l) {
                    l.set_state(State::Walking);
                }
            }
            _ => l.set_state(State::Walking),
        }
    }

    /// The block a miner breaks, for a miner whose feet are at `feet` with
    /// the block ahead in column `ahead` (x, z): the cells there and the
    /// segments cleared in each, from [`MINE_DROP`] below the feet to a
    /// walker's headroom above them; and its feet afterwards.
    fn mine_plan(feet: i32, ahead: [i32; 2]) -> (MineCells, i32) {
        // Feet are on a quarter-unit step. The cut's top: in Practice "Miner"
        // a miner starting on the plank (top 5¼) on the tower (top 5) leaves
        // ground above from its fourth block on (owner's observation).
        let feet = (feet + SUB / 8).div_euclid(SUB / 4) * (SUB / 4);
        let (bottom, top) = (feet - MINE_DROP, feet + HEAD_HEIGHT);
        let cells = (bottom.div_euclid(SUB)..=(top - 1).div_euclid(SUB))
            .map(|y| {
                let mask = (0..4)
                    .filter(|s| {
                        let lo = y * SUB + s * SUB / 4;
                        lo + SUB / 4 > bottom && lo < top
                    })
                    .fold(0u8, |m, s| m | 1 << s);
                ([ahead[0], y, ahead[1]], mask)
            })
            .collect();
        (cells, bottom)
    }

    /// The plan for the block ahead of miner `l`: the column after the one
    /// it stands in.
    fn mine_plan_for(l: &Lemming) -> (MineCells, i32) {
        let d = l.dir.delta();
        let c = Self::cell_of(l.pos);
        Self::mine_plan(l.pos[1], [c[0] + d[0], c[2] + d[2]])
    }

    /// Of a miner's plan: the cells and segments with terrain in them, and
    /// whether all of it gives way to mining towards `d` (no steel, no
    /// one-way block facing elsewhere).
    fn mine_present(&self, cells: &[([i32; 3], u8)], d: [i32; 3]) -> (MineCells, bool) {
        let present: Vec<_> = cells
            .iter()
            .filter_map(|&(c, mask)| {
                let m = self.world.block(c).map_or(0, |(b, _)| b.segments) & mask;
                (m != 0).then_some((c, m))
            })
            .collect();
        let ok = present
            .iter()
            .all(|&(c, _)| self.world.breakable_towards(c, Some(d)));
        (present, ok)
    }

    /// For a miner cracking the block ahead: the cells and segments being
    /// broken there, and the ticks since the cracks began (0 to
    /// [`MINE_CRACK_TICKS`] − 2). `None` otherwise.
    pub fn mine_target(&self, l: &Lemming) -> Option<(MineCells, u32)> {
        if l.state != State::Mining || !(1..MINE_CRACK_TICKS).contains(&l.state_ticks) {
            return None;
        }
        let (cells, _) = Self::mine_plan_for(l);
        let (present, _) = self.mine_present(&cells, l.dir.delta());
        Some((present, l.state_ticks - 1))
    }

    /// Mining (owner's observation): standing at the end of its block, the
    /// miner cracks the block ahead and breaks it, clearing it from half a
    /// unit below its feet up to the space its body needs (a blocky
    /// tunnel, half a unit down per block), then steps down into it, walks
    /// to its far end and starts again. It stops at steel, at a one-way
    /// block facing the other way, when nothing is left to remove, and at
    /// the level bottom; it falls if it breaks out into the open.
    fn mine(&mut self, l: &mut Lemming) {
        let d = l.dir.delta();
        let next = [l.pos[0] + d[0] * WALK_SPEED, l.pos[2] + d[2] * WALK_SPEED];
        // A stroke starts on tick 1 (the count goes up before each update),
        // [`MINE_STANDOFF`] before the end of the block stood on: one set to
        // mine further back walks there first.
        if l.state_ticks == 1 && Self::to_edge(l.pos, d) > MINE_STANDOFF {
            l.pos[0] = next[0];
            l.pos[2] = next[1];
            if Self::to_edge(l.pos, d) <= MINE_STANDOFF {
                l.pos = Self::mine_stand(l.pos, d);
            }
            if self.settle(l) {
                l.state_ticks = 0;
            }
            return;
        }
        if l.state_ticks == 1 {
            let (cells, bottom) = Self::mine_plan_for(l);
            let (present, ok) = self.mine_present(&cells, d);
            if present.is_empty() || !ok || bottom < GROUND {
                l.set_state(State::Walking);
                return;
            }
        }
        if l.state_ticks < MINE_CRACK_TICKS {
            return;
        }
        if l.state_ticks == MINE_CRACK_TICKS {
            let (cells, _) = Self::mine_plan_for(l);
            let (present, ok) = self.mine_present(&cells, d);
            if !ok {
                l.set_state(State::Walking);
                return;
            }
            for (c, mask) in present {
                self.world.remove_segments_towards(c, mask, Some(d));
            }
        }
        // A unit on, into the broken block, down onto what is left of it (a
        // segment down; out in the open, it falls), to the same place in it,
        // where the next stroke starts.
        l.pos[0] = next[0];
        l.pos[2] = next[1];
        let (ground, _) = self
            .world
            .surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
        if ground < l.pos[1] - MINE_DROP {
            l.start_falling();
            return;
        }
        l.pos[1] = ground;
        if l.state_ticks + 1 == MINE_CRACK_TICKS + (SUB / WALK_SPEED) as u32 {
            l.state_ticks = 0;
        }
    }

    /// Sub-units from `pos` to the far side of its cell along `d` (1 at the
    /// last position inside it).
    fn to_edge(pos: [i32; 3], d: [i32; 3]) -> i32 {
        let along = |v: i32, s: i32| match s.signum() {
            1 => (v.div_euclid(SUB) + 1) * SUB - v,
            -1 => v - v.div_euclid(SUB) * SUB + 1,
            _ => i32::MAX,
        };
        along(pos[0], d[0]).min(along(pos[2], d[2]))
    }

    /// Where a miner at `pos` facing `d` stands to mine the block ahead:
    /// [`MINE_STANDOFF`] before the far side of its cell.
    fn mine_stand(pos: [i32; 3], d: [i32; 3]) -> [i32; 3] {
        ahead_of(pos, d, Self::to_edge(pos, d) - MINE_STANDOFF)
    }

    /// For a digger: the cell under it, the segments it is digging out there
    /// (for a shaped block, the quarters its cut passes through), and the
    /// ticks since it started on them (0 to [`DIG_TICKS`] − 1). `None`
    /// otherwise or with nothing to dig.
    pub fn dig_target(&self, l: &Lemming) -> Option<([i32; 3], u8, u32)> {
        if l.state != State::Digging {
            return None;
        }
        let (c, b, _) = self.dig_cell(l)?;
        let cut = self.world.cut(c);
        let to = dig_cut(b, cut, l.pos[1] - c[1] * SUB);
        Some((
            c,
            segments_between(b.segments, to, cut),
            l.state_ticks % DIG_TICKS,
        ))
    }

    /// The cell under a digger's feet and its block.
    fn dig_cell(&self, l: &Lemming) -> Option<([i32; 3], l3d_formats::level::BlockCell, u8)> {
        let c = [
            l.pos[0].div_euclid(SUB),
            (l.pos[1] - 1).div_euclid(SUB),
            l.pos[2].div_euclid(SUB),
        ];
        let (b, f) = self.world.block(c)?;
        Some((c, b, f))
    }

    fn dig(&mut self, l: &mut Lemming) {
        if !l.state_ticks.is_multiple_of(DIG_TICKS) {
            return;
        }
        match self.dig_cell(l) {
            Some((c, b, f)) if f & flags::STEEL == 0 => {
                let cut = self.world.cut(c);
                let to = dig_cut(b, cut, l.pos[1] - c[1] * SUB);
                // A shaped block is shaved flat and keeps its slopes below
                // the cut: where a sloped underside leaves nothing under the
                // digger it falls, and the rest stays as a ledge (owner,
                // comparing Fun 2 with the original).
                if b.shape == 0 {
                    self.world
                        .remove_segments(c, segments_between(b.segments, to, 4));
                } else {
                    self.world.shave(c, to);
                }
                let (ground, on_block) = self.world.surface_below(l.pos[0], l.pos[1], l.pos[2]);
                if !on_block || ground < l.pos[1] - STEP_DOWN {
                    l.start_falling();
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
        [SUB / 8, HEAD_HEIGHT / 2, HEAD_HEIGHT - 1]
            .into_iter()
            .find_map(|h| self.world.deflector_normal([x, feet + h, z]))
    }

    fn walk(&mut self, l: &mut Lemming, me: usize, obstacles: &[Obstacle]) {
        let d = l.dir.delta();
        let landed = std::mem::take(&mut l.landed);
        // Exits: walking into an exit block through its doorway (the block's
        // +Z face, rotated with the block).
        // Probe as far ahead as the wall check below (one step plus REACH), so a
        // doorway is recognised before the walker would bounce off it.
        // On ice lemmings move about 3× as fast (observed: 3.1 ± 0.3).
        let speed = if l.state == State::Sliding {
            ICE_SPEED
        } else {
            WALK_SPEED
        };
        let reach = REACH + speed;
        // At the feet, or a step up (Taxing 4 "Poles Apart" has an exit
        // floor ¼ above the path leading to it).
        let exit = [SUB / 8, STEP_UP + SUB / 8].into_iter().find_map(|h| {
            self.world
                .block_at([
                    l.pos[0] + d[0] * reach,
                    l.pos[1] + h,
                    l.pos[2] + d[2] * reach,
                ])
                .filter(|b| b.id == EXIT_ID)
        });
        if let Some(b) = exit
            && l.dir == door_of(b).reverse()
        {
            l.set_state(State::Exiting);
            return;
        }
        let next = ahead_of(l.pos, d, speed);
        let axis = if d[0] != 0 { 0 } else { 2 };
        // Blockers and turners: walking into one redirects the walker.
        // Splitters turn walkers aside as they cross the cell centre
        // ([L3DEdit]: block 2 where non-solid), left and right in turn, each
        // heading in keeping its own turn: walkers along X start left, walkers
        // along Z start right, whatever the block's rotation
        // (docs/spec/behaviour.md, "Splitter").
        let here = Self::cell_of(l.pos);
        if let Some(s) = self.splitters.iter_mut().find(|(c, _)| *c == here) {
            let centre = mid(l.pos[axis]);
            if crosses(l.pos[axis], next[axis], centre, d[axis]) {
                l.pos[axis] = centre;
                let left = &mut s.1[l.dir as usize];
                l.dir = if *left {
                    l.dir.anticlockwise()
                } else {
                    l.dir.clockwise()
                };
                *left = !*left;
                return;
            }
        }
        for o in obstacles.iter().filter(|o| o.lemming != me) {
            let near = |p: [i32; 3]| {
                (p[0] - o.pos[0]).abs() < BLOCK_RADIUS
                    && (p[2] - o.pos[2]).abs() < BLOCK_RADIUS
                    && (p[1] - o.pos[1]).abs() < SUB / 2
            };
            let towards = (o.pos[axis] - l.pos[axis]) * d[axis] > 0;
            match o.turn_to {
                // Blockers turn walkers back at arm's length, and turn back
                // those already that close walking towards them (observed:
                // a blocker set in a tight pack turns the lemmings right
                // behind it; none walks through).
                None if near(next) && towards => {
                    l.dir = l.dir.reverse();
                    return;
                }
                // Turners turn only walkers going the way the turner was
                // walking when it was given; walkers meeting it from any
                // other side are not affected (owner). They are sent off as
                // they cross the centre of the turner's cell, so all leave
                // along its middle; one already past the centre walks on and
                // is lost (measured in the original's Practice "Turner" at
                // release rate 99). One landing past the centre is turned
                // where it lands (measured on a test level: hatch lemmings
                // landing about ¼ past the centre of a turner's cell).
                Some(to) if to != l.dir && l.dir == o.heading && Self::cell_of(o.pos) == here => {
                    let centre = mid(l.pos[axis]);
                    let past = (centre - l.pos[axis]) * d[axis] <= 0;
                    if landed && past {
                        l.dir = to;
                        return;
                    }
                    if crosses(l.pos[axis], next[axis], centre, d[axis]) {
                        l.pos[axis] = centre;
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
        if let Some(n) = self
            .deflector_across(ahead[0], l.pos[1], ahead[1])
            .filter(|&n| facing(n))
        {
            if self.deflector_across(next[0], l.pos[1], next[2]).is_some() {
                // A climber climbs the face instead (the original: a climber
                // landing from Taxing 20 "Death Slide"'s hatch climbed the
                // deflector column the walkers are turned at). As at a wall,
                // only a face reaching down to its feet.
                if l.climber
                    && l.pending.is_none()
                    && self.world.solid([ahead[0], l.pos[1] + SUB / 8, ahead[1]])
                {
                    l.set_state(State::Climbing);
                } else if l.pending == Some(State::Bashing) {
                    // A basher given inside the deflector's cell takes the
                    // skill up at the face, as at a wall, and bashes it
                    // (inferred: Practice "Deflector" asks for its corner to
                    // be bashed, and bashers that reach the cell's edge first
                    // are turned by the face instead). It stands an eighth
                    // of a unit before the face, which crosses the middle of
                    // the cell.
                    for a in [0, 2] {
                        if d[a] != 0 {
                            l.pos[a] = mid(l.pos[a]) - d[a] * (SUB / 8);
                        }
                    }
                    l.pending = None;
                    l.set_state(State::Bashing);
                } else {
                    l.dir = l.dir.deflected(n);
                }
                return;
            }
            l.pos[0] = next[0];
            l.pos[2] = next[2];
            return;
        }
        // The surface where the feet will be after this step: a step up of
        // more than STEP_UP there is a wall. (Probing further ahead would
        // read a 45° ramp as a wall.)
        let (surface, _) = self
            .world
            .surface_below(next[0], l.pos[1] + STEP_UP + 1, next[2]);
        // Body clearance ahead: solid at step height above that surface is a
        // wall (a ramp rises at most REACH over the probe distance, which stays
        // below this), and a ceiling lower than the walker's head blocks too.
        // Bricks are floors only, never walls (owner's recollection: walkers
        // meeting a staircase from the side step onto a low step or pass
        // under).
        let wall = |w: &World, y: i32| w.terrain_solid([ahead[0], y, ahead[1]]);
        // Stepping off an edge, the walker is still at its own height when
        // it moves on (then falls), so that is where it needs room: walls
        // down by a lower surface don't turn it round.
        let body = if surface < l.pos[1] - STEP_DOWN {
            l.pos[1]
        } else {
            surface
        };
        let blocked = surface > l.pos[1] + STEP_UP
            || wall(&self.world, body + STEP_UP + 2)
            || wall(&self.world, body + HEAD_HEIGHT - 1);
        if blocked {
            // Climbers climb walls that reach down to their feet; at an
            // overhang they turn like walkers (provisional: lets the
            // climbers of "Candyland Climber" reach its exit). A climber with
            // a skill pending takes the skill up at the wall instead
            // (inferred: Tricky 3 "This Is the Army" has a climber mine
            // through a wall it walks up to).
            if l.climber
                && l.pending.is_none()
                && self.world.solid([ahead[0], l.pos[1] + SUB / 8, ahead[1]])
            {
                l.set_state(State::Climbing);
            } else {
                l.dir = l.dir.reverse();
            }
            return;
        }
        l.pos[0] = next[0];
        l.pos[2] = next[2];
        let (ground, on_block) = self
            .world
            .surface_below(l.pos[0], l.pos[1] + STEP_UP, l.pos[2]);
        if ground >= l.pos[1] - STEP_DOWN {
            let stepped = (l.pos[1] - ground).max(0);
            l.pos[1] = ground;
            let (ground_flags, wet) = self.underfoot(l.pos, on_block);
            if wet {
                l.set_state(self.sunk());
            } else if !self.bounce(l, stepped) {
                // Slippery tops and floors make walkers slide; leaving them,
                // they walk.
                let slippery = ground_flags.is_some_and(|f| f & flags::SLIPPERY != 0)
                    || (!on_block && self.world.floor_slippery(l.pos[0], l.pos[2]));
                l.state = if slippery {
                    State::Sliding
                } else {
                    State::Walking
                };
            }
        } else {
            l.start_falling();
        }
    }
}

/// Grid extents in sub-units, for callers that clamp cameras or cursors.
pub const EXTENT: [i32; 3] = [
    SIZE_X as i32 * SUB,
    SIZE_Y as i32 * SUB,
    SIZE_Z as i32 * SUB,
];

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
