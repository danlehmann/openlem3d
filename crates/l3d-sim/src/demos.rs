//! Solutions for the Practice levels (and Fun 1), found in this simulation.
//! Played back, they are our counterpart of the original's Practice demos
//! (whose recordings, `REPLAYS/REPLAY.080`–`099`, are the original's own
//! input and can't drive this simulation).

use l3d_formats::blk::BlockSet;
use l3d_formats::level::Level;

use crate::{Command, Simulation, Skill};

/// How a solution gives out skills.
#[derive(Debug, Clone, Copy)]
pub enum Plan {
    /// `(tick, lemming, skill id, turner side)` assignments, after that many
    /// ticks; the side is `Some(true)` for clockwise.
    At(&'static [(u64, usize, u8, Option<bool>)]),
    /// The skill to every lemming as soon as it appears.
    Each(u8),
}

/// A known solution: level file number, plan, and how many it saves (0:
/// at least the level's requirement).
pub struct Solution {
    pub level: u32,
    pub plan: Plan,
    pub saves: u32,
}

const fn at(level: u32, steps: &'static [(u64, usize, u8, Option<bool>)]) -> Solution {
    Solution { level, plan: Plan::At(steps), saves: 0 }
}

pub const SOLUTIONS: &[Solution] = &[
    at(80, &[(140, 0, 0, None)]),
    at(81, &[(175, 0, 1, Some(true))]),
    at(82, &[(125, 0, 2, None)]),
    at(83, &[(185, 0, 3, None)]),
    at(84, &[(235, 0, 4, None)]),
    at(85, &[(150, 0, 5, None)]),
    at(86, &[(100, 0, 6, None)]),
    Solution { level: 87, plan: Plan::Each(7), saves: 0 },
    Solution { level: 88, plan: Plan::Each(8), saves: 0 },
    at(89, &[(66, 0, 4, None)]),
    at(90, &[(410, 0, 1, Some(true))]),
    at(91, &[(570, 0, 4, None)]),
    at(92, &[(100, 0, 3, None), (277, 0, 3, None)]),
    at(93, &[(122, 0, 4, None)]),
    at(94, &[(190, 0, 1, Some(false))]),
    at(95, &[(245, 0, 1, Some(true))]),
    at(96, &[(200, 0, 1, Some(false))]),
    at(97, &[(170, 0, 0, None)]),
    Solution { level: 98, plan: Plan::At(&[(343, 1, 1, Some(false))]), saves: 18 },
    at(99, &[(1124, 0, 4, None)]),
    at(0, &[(1943, 0, 1, Some(false))]),
];

pub fn solution(level: u32) -> Option<&'static Solution> {
    SOLUTIONS.iter().find(|s| s.level == level)
}

/// Plays a solution through and returns the finished simulation, whose
/// `log` replays it.
pub fn play(s: &Solution, level: &Level, blocks: &BlockSet) -> Simulation {
    let mut sim = Simulation::new(level, blocks);
    while !sim.finished() {
        sim.step();
        match s.plan {
            Plan::At(steps) => {
                let now = sim.tick;
                for &(t, i, id, side) in steps.iter().filter(|(t, ..)| *t == now) {
                    let ok = match (side, sim.lemmings.get(i).map(|l| l.dir)) {
                        (Some(cw), Some(d)) => sim.assign_turner(i, if cw { d.clockwise() } else { d.anticlockwise() }),
                        _ => Skill::from_id(id).is_some_and(|skill| sim.assign(i, skill)),
                    };
                    debug_assert!(ok, "LEVEL.{:03}: assignment at tick {t} rejected", s.level);
                }
            }
            Plan::Each(id) => {
                if let Some(skill) = Skill::from_id(id) {
                    for i in 0..sim.lemmings.len() {
                        sim.assign(i, skill);
                    }
                }
            }
        }
    }
    sim
}

/// The commands of level `n`'s demo, if it has one.
pub fn demo(n: u32, level: &Level, blocks: &BlockSet) -> Option<Vec<(u64, Command)>> {
    solution(n).map(|s| play(s, level, blocks).log)
}
