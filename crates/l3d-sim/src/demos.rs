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
    at(83, &[(182, 0, 3, None)]),
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
        &[(136, 0, 3, None), (745, 0, 3, None), (916, 0, 3, None)],
    ),
    at(
        3,
        &[
            (831, 0, 6, None),
            (1231, 0, 6, None),
            (1661, 0, 1, Some(true)),
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
        &[(104, 0, 2, None), (537, 1, 2, None), (1207, 2, 2, None)],
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
            (758, 3, 3, None),
            (1202, 4, 1, Some(false)),
            (1458, 5, 3, None),
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
            (180, 0, 4, None),
            (199, 1, 4, None),
            (218, 2, 4, None),
            (237, 3, 4, None),
            (422, 0, 3, None),
            (441, 1, 3, None),
            (460, 2, 3, None),
            (479, 3, 3, None),
            (705, 0, 2, None),
            (724, 1, 2, None),
            (743, 2, 2, None),
            (762, 3, 2, None),
        ],
    ),
    at(
        62,
        &[
            (283, 0, 1, Some(true)),
            (437, 1, 1, Some(true)),
            (655, 2, 1, Some(false)),
            (915, 3, 1, Some(false)),
            (1020, 4, 7, None),
            (1090, 5, 7, None),
            (1160, 4, 0, None),
            (1283, 5, 4, None),
            (1473, 5, 1, Some(false)),
            (1665, 6, 1, Some(false)),
            (1779, 7, 1, Some(false)),
        ],
    ),
    at(
        40,
        &[
            (10, 0, 1, Some(true)),
            (74, 1, 1, Some(true)),
            (181, 2, 1, Some(false)),
            (445, 3, 1, Some(false)),
            (940, 4, 1, Some(false)),
            (1465, 5, 1, Some(false)),
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
    at(21, &[(23, 0, 0, None), (320, 1, 0, None)]),
    at(
        22,
        &[
            (2, 0, 7, None),
            (2, 0, 8, None),
            (83, 1, 7, None),
            (83, 1, 8, None),
            (606, 1, 2, None),
            (700, 0, 5, None),
            (925, 2, 3, None),
            (1915, 2, 3, None),
            (2040, 2, 4, None),
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
            (543, 2, 1, Some(false)),
            (656, 4, 1, Some(true)),
            (690, 3, 1, Some(true)),
            (887, 5, 1, Some(true)),
            (980, 6, 1, Some(false)),
            (982, 8, 3, None),
            (1130, 8, 3, None),
            (1131, 7, 1, Some(false)),
            (1280, 10, 1, Some(false)),
            (1320, 9, 1, Some(false)),
            (1615, 14, 1, Some(true)),
        ],
    ),
    Solution {
        level: 60,
        plan: Plan::Rated(
            &[(770, 99)],
            &[
                (2, 0, 7, None),
                (385, 0, 6, None),
                (603, 0, 3, None),
                (700, 1, 7, None),
                (750, 2, 7, None),
                (856, 2, 6, None),
                (960, 2, 2, None),
                (1065, 0, 6, None),
                (1362, 1, 2, None),
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
            (704, 2, 1, Some(true)),
            (856, 3, 3, None),
            (1171, 4, 1, Some(true)),
            (1326, 5, 1, Some(true)),
        ],
    ),
    // Fun levels, play-tested in the simulation; each follows the published
    // route unless noted.
    at(1, &[(231, 0, 6, None)]),
    at(
        8,
        &[
            (55, 0, 1, Some(true)),
            (201, 1, 1, Some(false)),
            (317, 2, 1, Some(false)),
            (464, 3, 1, Some(true)),
            (611, 4, 1, Some(false)),
            (704, 5, 3, None),
            (873, 6, 1, Some(false)),
            (987, 7, 1, Some(true)),
        ],
    ),
    at(
        9,
        &[
            (45, 0, 3, None),
            (195, 0, 3, None),
            (356, 0, 1, Some(true)),
            (454, 2, 1, Some(false)),
        ],
    ),
    at(
        10,
        &[
            (87, 0, 0, None),
            (362, 1, 1, Some(false)),
            (702, 2, 0, None),
            (1009, 3, 0, None),
            (1254, 4, 0, None),
            (1529, 5, 1, Some(true)),
            (1805, 6, 0, None),
            (2080, 7, 1, Some(false)),
            (2260, 8, 1, Some(true)),
        ],
    ),
    Solution {
        level: 11,
        plan: Plan::Rated(
            &[(650, 99)],
            &[
                (120, 0, 2, None),
                (207, 1, 1, Some(true)),
                (500, 2, 2, None),
                (620, 3, 2, None),
                (785, 4, 2, None),
                (1114, 5, 2, None),
            ],
        ),
        saves: 0,
    },
    at(
        12,
        &[
            (125, 0, 1, Some(false)),
            (345, 1, 1, Some(false)),
            (557, 2, 1, Some(false)),
            (777, 3, 1, Some(false)),
            (994, 4, 1, Some(false)),
            (1214, 5, 1, Some(false)),
            (1368, 15, 8, None),
            (1445, 6, 1, Some(false)),
            (1459, 16, 8, None),
            (1550, 17, 8, None),
            (1555, 7, 3, None),
            (1641, 18, 8, None),
            (1732, 19, 8, None),
            (1750, 8, 1, Some(false)),
            (1823, 20, 8, None),
            (1914, 21, 8, None),
            (1968, 9, 1, Some(false)),
            (2005, 22, 8, None),
            (2068, 10, 3, None),
            (2096, 23, 8, None),
            (2187, 24, 8, None),
            (2278, 11, 1, Some(false)),
            (2278, 25, 8, None),
            (2369, 26, 8, None),
            (2460, 27, 8, None),
            (2498, 12, 1, Some(false)),
            (2551, 28, 8, None),
            (2642, 29, 8, None),
            (2717, 13, 1, Some(false)),
            (2733, 30, 8, None),
            (2824, 31, 8, None),
            (2871, 14, 1, Some(false)),
            (2915, 32, 8, None),
            (3006, 33, 8, None),
            (3097, 34, 8, None),
        ],
    ),
    Solution {
        level: 13,
        plan: Plan::Rated(
            &[(600, 49)],
            &[
                (80, 0, 1, Some(false)),
                (203, 1, 1, Some(true)),
                (318, 2, 1, Some(true)),
                (508, 3, 1, Some(true)),
                (643, 4, 1, Some(true)),
                (788, 5, 1, Some(true)),
                (990, 6, 1, Some(true)),
                (1114, 7, 1, Some(false)),
            ],
        ),
        saves: 0,
    },
    at(
        14,
        &[
            (197, 0, 1, Some(false)),
            (402, 1, 1, Some(true)),
            (582, 2, 1, Some(false)),
            (697, 3, 4, None),
            (1182, 4, 1, Some(false)),
        ],
    ),
    Solution {
        level: 15,
        plan: Plan::Rated(
            &[(400, 79)],
            &[
                (134, 0, 3, None),
                (385, 0, 1, Some(false)),
                (463, 1, 1, Some(true)),
                (671, 2, 1, Some(true)),
                (816, 3, 1, Some(true)),
                (1290, 4, 6, None),
            ],
        ),
        saves: 0,
    },
    at(
        16,
        &[
            (112, 0, 0, None),
            (113, 0, 8, None),
            (1345, 9, 4, None),
            (2040, 0, 4, None),
        ],
    ),
    // The published route (mining round the ring), at the base release
    // rate.
    at(
        17,
        &[
            (15, 0, 5, None),
            (258, 0, 1, Some(true)),
            (267, 1, 5, None),
            (517, 1, 1, Some(true)),
            (527, 2, 5, None),
            (777, 2, 1, Some(true)),
            (787, 3, 5, None),
            (1036, 3, 1, Some(true)),
            (1047, 4, 5, None),
            (1296, 4, 1, Some(true)),
            (1307, 5, 5, None),
            (1557, 5, 1, Some(true)),
            (1567, 6, 5, None),
            (1817, 6, 1, Some(true)),
            (1827, 7, 5, None),
            (1995, 7, 1, Some(true)),
        ],
    ),
    at(
        18,
        &[
            (75, 0, 1, Some(false)),
            (226, 1, 6, None),
            (511, 1, 1, Some(false)),
            (681, 3, 1, Some(false)),
            (886, 4, 1, Some(false)),
            (1061, 5, 1, Some(false)),
            (1118, 6, 4, None),
            (1311, 6, 1, Some(true)),
            (1441, 7, 1, Some(true)),
            (1491, 8, 4, None),
            (1738, 8, 1, Some(true)),
            (1926, 9, 1, Some(true)),
            (2106, 10, 1, Some(true)),
        ],
    ),
    at(
        19,
        &[
            (60, 0, 1, Some(false)),
            (330, 1, 1, Some(true)),
            (625, 2, 1, Some(true)),
            (735, 3, 3, None),
            (937, 4, 1, Some(false)),
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
