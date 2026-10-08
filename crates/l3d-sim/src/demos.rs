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
    /// `(tick, lemming, skill id, turner side)` assignments, that many ticks
    /// after the hatches have opened; the side is `Some(true)` for clockwise.
    At(&'static [(u64, usize, u8, Option<bool>)]),
    /// Like [`Plan::At`], with release-rate changes `(tick, change)`, each
    /// made before that tick's assignments.
    Rated(
        &'static [(u64, i32)],
        &'static [(u64, usize, u8, Option<bool>)],
    ),
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
    Solution {
        level,
        plan: Plan::At(steps),
        saves: 0,
    }
}

pub const SOLUTIONS: &[Solution] = &[
    at(80, &[(140, 0, 0, None)]),
    at(81, &[(150, 0, 1, Some(true))]),
    at(82, &[(131, 0, 2, None)]),
    at(83, &[(185, 0, 3, None)]),
    at(84, &[(235, 0, 4, None)]),
    at(85, &[(150, 0, 5, None)]),
    at(86, &[(100, 0, 6, None)]),
    Solution {
        level: 87,
        plan: Plan::Each(7),
        saves: 0,
    },
    Solution {
        level: 88,
        plan: Plan::Each(8),
        saves: 0,
    },
    at(89, &[(41, 1, 4, None)]),
    at(90, &[(406, 0, 1, Some(true))]),
    at(91, &[(538, 0, 4, None)]),
    at(92, &[(100, 0, 3, None), (265, 0, 3, None)]),
    at(93, &[(122, 0, 4, None)]),
    at(94, &[(182, 0, 1, Some(false))]),
    at(95, &[(239, 0, 1, Some(true))]),
    at(96, &[(167, 0, 1, Some(false))]),
    at(97, &[(170, 0, 0, None)]),
    Solution {
        level: 98,
        plan: Plan::At(&[(253, 0, 1, Some(false))]),
        saves: 18,
    },
    at(99, &[(1124, 0, 4, None)]),
    at(0, &[(1409, 0, 1, Some(false))]),
    // Climbers; the first, blown up on the candy cane, opens a way through
    // for the next, which bashes the cane's one-way foot from the far side
    // (the owner's route).
    at(
        6,
        &[
            (2, 0, 7, None),
            (53, 1, 7, None),
            (104, 2, 7, None),
            (155, 3, 7, None),
            (240, 0, 2, None),
            (436, 1, 4, None),
        ],
    ),
    // Found by play-testing in the simulation; each looks like the designed
    // route.
    at(
        2,
        &[(150, 0, 3, None), (760, 0, 3, None), (931, 0, 3, None)],
    ),
    at(
        3,
        &[
            (780, 0, 6, None),
            (1180, 0, 6, None),
            (1610, 0, 1, Some(true)),
        ],
    ),
    at(
        4,
        &[
            (274, 0, 1, Some(false)),
            (660, 1, 1, Some(false)),
            (858, 2, 1, Some(true)),
        ],
    ),
    at(
        5,
        &[(104, 0, 2, None), (520, 1, 2, None), (1190, 2, 2, None)],
    ),
    at(
        7,
        &[
            (470, 0, 1, Some(false)),
            (615, 1, 3, None),
            (779, 1, 3, None),
            (840, 3, 1, Some(false)),
        ],
    ),
    at(
        20,
        &[
            (5, 0, 7, None),
            (56, 1, 7, None),
            (107, 2, 7, None),
            (158, 3, 7, None),
            (209, 4, 7, None),
            (260, 5, 7, None),
            (311, 6, 7, None),
            (330, 0, 1, Some(false)),
            (362, 7, 7, None),
            (413, 8, 7, None),
            (464, 9, 7, None),
        ],
    ),
    at(
        23,
        &[
            (130, 0, 1, Some(true)),
            (270, 1, 5, None),
            (650, 2, 1, Some(false)),
            (766, 3, 3, None),
            (1210, 4, 1, Some(false)),
            (1466, 5, 3, None),
        ],
    ),
    at(
        24,
        &[
            (112, 0, 2, None),
            (152, 1, 2, None),
            (875, 6, 1, Some(true)),
            (1010, 3, 2, None),
            (1585, 7, 1, Some(true)),
        ],
    ),
    at(
        42,
        &[
            (190, 0, 4, None),
            (209, 1, 4, None),
            (228, 2, 4, None),
            (247, 3, 4, None),
            (432, 0, 3, None),
            (451, 1, 3, None),
            (470, 2, 3, None),
            (489, 3, 3, None),
            (715, 0, 2, None),
            (734, 1, 2, None),
            (753, 2, 2, None),
            (772, 3, 2, None),
        ],
    ),
    at(
        62,
        &[
            (297, 0, 1, Some(true)),
            (432, 1, 4, None),
            (625, 1, 1, Some(true)),
            (726, 2, 3, None),
            (966, 2, 3, None),
            (1100, 3, 1, Some(false)),
            (1310, 4, 1, Some(false)),
            (1437, 5, 1, Some(false)),
        ],
    ),
    at(
        40,
        &[
            (407, 0, 1, Some(false)),
            (662, 5, 1, Some(false)),
            (733, 2, 1, Some(false)),
            (989, 7, 1, Some(false)),
            (1310, 4, 1, Some(false)),
        ],
    ),
    at(
        61,
        &[
            (2, 0, 7, None),
            (53, 1, 7, None),
            (248, 0, 0, None),
            (342, 1, 4, None),
            (420, 0, 2, None),
            (560, 6, 1, Some(true)),
            (992, 1, 0, None),
            (1050, 4, 7, None),
            (1395, 4, 4, None),
            (1440, 1, 2, None),
            (1592, 12, 1, Some(false)),
            (1990, 4, 0, None),
            (1996, 15, 7, None),
            (2142, 15, 4, None),
            (2220, 4, 2, None),
        ],
    ),
    // Published human solutions (walkthrough sites), timed in the
    // simulation.
    at(21, &[(30, 0, 0, None), (320, 1, 0, None)]),
    at(
        22,
        &[
            (2, 0, 7, None),
            (2, 0, 8, None),
            (83, 1, 7, None),
            (83, 1, 8, None),
            (700, 0, 5, None),
            (1875, 2, 3, None),
            (2039, 2, 4, None),
        ],
    ),
    at(
        41,
        &[
            (160, 2, 1, Some(false)),
            (300, 4, 1, Some(true)),
            (330, 6, 7, None),
            (410, 1, 0, None),
            (432, 8, 7, None),
            (534, 10, 7, None),
            (580, 6, 1, Some(true)),
            (590, 2, 2, None),
            (705, 8, 1, Some(true)),
            (830, 10, 4, None),
            (840, 6, 2, None),
            (841, 8, 2, None),
            (885, 12, 0, None),
            (1320, 16, 4, None),
            (1342, 14, 1, Some(false)),
        ],
    ),
    at(
        44,
        &[
            (250, 0, 1, Some(false)),
            (267, 1, 1, Some(true)),
            (545, 2, 1, Some(false)),
            (688, 3, 1, Some(true)),
            (890, 5, 1, Some(false)),
            (970, 6, 1, Some(false)),
            (1385, 12, 1, Some(false)),
            (1745, 4, 1, Some(true)),
            (1755, 8, 1, Some(true)),
            (2205, 22, 1, Some(true)),
            (5590, 9, 1, Some(true)),
            (5612, 41, 3, None),
            (5651, 25, 3, None),
            (5699, 30, 1, Some(false)),
        ],
    ),
    Solution {
        level: 60,
        plan: Plan::Rated(
            &[(770, 99)],
            &[
                (2, 0, 7, None),
                (398, 0, 6, None),
                (603, 0, 3, None),
                (700, 1, 7, None),
                (750, 2, 7, None),
                (856, 2, 6, None),
                (956, 0, 2, None),
                (960, 2, 2, None),
                (1450, 1, 6, None),
            ],
        ),
        saves: 0,
    },
    at(
        27,
        &[
            (410, 1, 2, None),
            (420, 0, 6, None),
            (886, 2, 4, None),
            (1080, 0, 0, None),
        ],
    ),
    at(
        7,
        &[
            (160, 0, 4, None),
            (352, 0, 1, Some(true)),
            (723, 2, 1, Some(true)),
            (875, 3, 3, None),
            (1190, 4, 1, Some(true)),
            (1345, 5, 1, Some(true)),
        ],
    ),
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
        let now = sim.tick;
        let due = |t: u64| t + crate::HATCH_OPEN_TICKS as u64 == now;
        match s.plan {
            Plan::At(steps) | Plan::Rated(_, steps) => {
                if let Plan::Rated(rates, _) = s.plan {
                    for &(_, change) in rates.iter().filter(|(t, _)| due(*t)) {
                        sim.adjust_release_rate(change);
                    }
                }
                for &(t, i, id, side) in steps.iter().filter(|(t, ..)| due(*t)) {
                    let ok = match (side, sim.lemmings.get(i).map(|l| l.dir)) {
                        (Some(cw), Some(d)) => {
                            sim.assign_turner(i, if cw { d.clockwise() } else { d.anticlockwise() })
                        }
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
