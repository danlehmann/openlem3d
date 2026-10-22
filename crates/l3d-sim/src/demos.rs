//! Solutions for the Practice levels (and Fun 1), found in this simulation.
//! Played back, they are our counterpart of the original's Practice demos
//! (whose recordings, `REPLAYS/REPLAY.080`–`099`, are the original's own
//! input and can't drive this simulation).

use l3d_formats::blk::BlockSet;
use l3d_formats::level::Level;

use crate::{Command, Simulation, Skill};

/// A skill assignment `(tick, lemming, skill id, turner side)`; the side is
/// `Some(true)` for clockwise.
pub type Step = (u64, usize, u8, Option<bool>);

/// A release-rate change `(tick, change)`.
pub type RateStep = (u64, i32);

/// How a solution gives out skills.
#[derive(Debug, Clone, Copy)]
pub enum Plan {
    /// Assignments, ticks counted from the hatches opening.
    At(&'static [Step]),
    /// Like [`Plan::At`], with release-rate changes, each made before that
    /// tick's assignments.
    Rated(&'static [RateStep], &'static [Step]),
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

impl Solution {
    /// How many lemmings this solution must save on `level`, the level it
    /// solves.
    pub fn need(&self, level: &Level) -> u32 {
        if self.saves > 0 {
            self.saves
        } else {
            level.save_requirement as u32
        }
    }
}

const fn at(level: u32, steps: &'static [Step]) -> Solution {
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
            (223, 0, 6, None),
            (923, 0, 6, None),
            (1613, 0, 1, Some(true)),
        ],
    ),
    at(
        4,
        &[
            (162, 0, 1, Some(false)),
            (344, 1, 1, Some(true)),
            (507, 2, 1, Some(false)),
            (620, 3, 1, Some(false)),
            (734, 4, 1, Some(true)),
            (935, 5, 1, Some(false)),
            (1094, 6, 1, Some(true)),
        ],
    ),
    at(
        5,
        &[
            (112, 0, 2, None),
            (545, 1, 2, None),
            (806, 2, 2, None),
            (1266, 3, 2, None),
        ],
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
            (770, 1, 1, Some(false)),
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
            (1010, 3, 2, None),
            (1156, 2, 1, Some(true)),
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
        40,
        &[
            (415, 0, 1, Some(false)),
            (465, 1, 1, Some(true)),
            (740, 2, 1, Some(false)),
            (793, 3, 1, Some(true)),
            (1324, 4, 1, Some(false)),
            (1430, 5, 1, Some(false)),
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
            (14, 0, 7, None),
            (14, 0, 8, None),
            (95, 1, 7, None),
            (95, 1, 8, None),
            (618, 1, 2, None),
            (712, 0, 5, None),
            (937, 2, 3, None),
            (1913, 2, 3, None),
            (2038, 2, 4, None),
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
            (252, 0, 1, Some(false)),
            (269, 1, 1, Some(true)),
            (545, 2, 1, Some(false)),
            (658, 4, 1, Some(true)),
            (692, 3, 1, Some(true)),
            (889, 5, 1, Some(true)),
            (982, 6, 1, Some(false)),
            (984, 8, 3, None),
            (1132, 8, 3, None),
            (1133, 7, 1, Some(false)),
            (1282, 10, 1, Some(false)),
            (1322, 9, 1, Some(false)),
            (1617, 14, 1, Some(true)),
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
            (400, 0, 6, None),
            (411, 1, 2, None),
            (866, 2, 4, None),
            (1060, 0, 0, None),
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
    // Fun 2 by a route of our own (saves 75 of 70).
    Solution {
        level: 1,
        plan: Plan::Rated(
            &[(187, 99)],
            &[
                (12, 0, 8, None),
                (57, 2, 8, None),
                (78, 3, 8, None),
                (120, 5, 8, None),
                (141, 6, 8, None),
                (162, 7, 8, None),
                (183, 8, 8, None),
                (252, 1, 6, None),
                (340, 9, 8, None),
                (340, 79, 8, None),
                (524, 4, 0, None),
                (600, 0, 6, None),
                (690, 26, 0, None),
                (750, 42, 0, None),
            ],
        ),
        saves: 0,
    },
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
            (50, 0, 3, None),
            (200, 0, 3, None),
            (361, 0, 1, Some(true)),
            (1335, 2, 1, Some(true)),
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
    // Tricky levels, play-tested in the simulation; Tricky 15 "Lemlab" by a
    // route of our own.
    at(
        32,
        &[
            (190, 0, 1, Some(false)),
            (326, 1, 1, Some(true)),
            (557, 2, 1, Some(false)),
            (755, 3, 1, Some(false)),
            (986, 4, 1, Some(false)),
            (1121, 5, 1, Some(true)),
            (1256, 6, 1, Some(false)),
            (1391, 7, 1, Some(true)),
            (1526, 8, 1, Some(true)),
            (1758, 9, 1, Some(true)),
            (2088, 10, 1, Some(false)),
            (2222, 11, 1, Some(false)),
        ],
    ),
    at(
        34,
        &[
            (220, 0, 1, Some(true)),
            (338, 1, 5, None),
            (460, 1, 0, None),
            (618, 2, 1, Some(false)),
            (760, 3, 3, None),
            (944, 4, 6, None),
            (1170, 4, 1, Some(false)),
            (1387, 7, 1, Some(false)),
            (1508, 8, 3, None),
        ],
    ),
    at(
        35,
        &[
            (220, 0, 1, Some(false)),
            (597, 1, 3, None),
            (706, 1, 1, Some(true)),
            (1012, 2, 1, Some(true)),
        ],
    ),
    at(
        36,
        &[
            (252, 0, 1, Some(false)),
            (418, 1, 1, Some(true)),
            (590, 3, 7, None),
            (642, 3, 6, None),
            (673, 3, 2, None),
            (1140, 8, 1, Some(true)),
            (1305, 9, 1, Some(true)),
        ],
    ),
    Solution {
        level: 37,
        plan: Plan::Rated(
            &[(900, 99)],
            &[
                (285, 0, 4, None),
                (1180, 0, 7, None),
                (1280, 3, 4, None),
                (1470, 4, 4, None),
                (1699, 0, 6, None),
            ],
        ),
        saves: 0,
    },
    at(
        38,
        &[
            (130, 0, 1, Some(false)),
            (275, 1, 3, None),
            (490, 2, 1, Some(true)),
            (570, 3, 3, None),
            (852, 4, 1, Some(true)),
            (1058, 5, 1, Some(false)),
        ],
    ),
    Solution {
        level: 39,
        plan: Plan::Rated(
            &[(450, 99)],
            &[
                (640, 0, 1, Some(true)),
                (1185, 1, 1, Some(true)),
                (2162, 2, 3, None),
                (2862, 3, 6, None),
                (3943, 3, 1, Some(true)),
                (4141, 4, 1, Some(true)),
                (4308, 5, 1, Some(false)),
            ],
        ),
        saves: 0,
    },
    // Taxing levels, by the published walkthroughs (Taxing 13 "Motherboard"
    // by xeye's route).
    at(
        51,
        &[
            (885, 79, 3, None),
            (1062, 79, 3, None),
            (1254, 79, 3, None),
            (1428, 79, 3, None),
            (1540, 0, 7, None),
            (1745, 79, 3, None),
            (1903, 0, 3, None),
            (2190, 79, 4, None),
            (2358, 0, 4, None),
        ],
    ),
    at(
        52,
        &[
            (590, 0, 0, None),
            (702, 1, 1, Some(true)),
            (755, 2, 3, None),
            (978, 3, 1, Some(true)),
        ],
    ),
    at(
        54,
        &[
            (110, 1, 7, None),
            (460, 0, 4, None),
            (770, 1, 3, None),
            (947, 0, 5, None),
            (980, 2, 6, None),
        ],
    ),
    at(
        55,
        &[
            (110, 0, 2, None),
            (620, 1, 2, None),
            (1587, 2, 2, None),
            (1862, 3, 2, None),
            (2240, 4, 2, None),
            (2690, 5, 1, Some(true)),
            (2770, 6, 3, None),
        ],
    ),
    Solution {
        level: 56,
        plan: Plan::Rated(&[(0, 16), (20, 99)], &[(382, 0, 4, None)]),
        saves: 0,
    },
    Solution {
        level: 57,
        plan: Plan::Rated(
            &[(963, 29), (1050, -29)],
            &[
                (68, 0, 1, Some(true)),
                (150, 1, 0, None),
                (375, 2, 1, Some(false)),
                (595, 4, 1, Some(false)),
                (890, 6, 1, Some(true)),
                (1078, 8, 1, Some(true)),
                (1330, 10, 1, Some(false)),
                (1520, 12, 1, Some(false)),
                (1735, 78, 2, None),
                (1742, 14, 1, Some(true)),
                (1900, 16, 1, Some(true)),
                (2089, 76, 2, None),
                (2090, 18, 1, Some(false)),
                (2248, 20, 1, Some(false)),
                (2347, 74, 2, None),
                (2406, 22, 1, Some(true)),
                (2532, 24, 1, Some(true)),
                (2605, 72, 2, None),
                (2662, 26, 1, Some(false)),
                (2767, 70, 2, None),
                (2786, 28, 1, Some(false)),
                (2880, 30, 1, Some(true)),
                (2929, 68, 2, None),
                (2995, 66, 2, None),
                (3051, 64, 2, None),
            ],
        ),
        saves: 0,
    },
    // Taxing 20 "Death Slide", partly by the published walkthrough.
    at(
        59,
        &[
            (1, 0, 7, None),
            (82, 1, 7, None),
            (385, 0, 1, Some(true)),
            (500, 1, 3, None),
            (635, 1, 5, None),
            (700, 2, 3, None),
            (1275, 2, 1, Some(true)),
            (1470, 3, 0, None),
            (1600, 4, 3, None),
        ],
    ),
    // Mayhem levels, play-tested in the simulation; Mayhem 8 "Across the
    // Network" by a route of our own, Mayhem 4 "Driving Range" partly.
    at(
        66,
        &[
            (240, 1, 7, None),
            (330, 0, 1, Some(true)),
            (410, 1, 6, None),
            (490, 2, 3, None),
            (569, 5, 1, Some(false)),
            (725, 7, 3, None),
            (816, 10, 1, Some(true)),
            (885, 7, 3, None),
            (999, 15, 1, Some(true)),
            (1010, 17, 1, Some(false)),
            (1214, 20, 1, Some(false)),
            (1275, 25, 3, None),
            (1930, 1, 5, None),
            (2530, 4, 5, None),
        ],
    ),
    at(
        63,
        &[
            (275, 0, 8, None),
            (280, 0, 7, None),
            (430, 1, 4, None),
            (630, 0, 4, None),
            (852, 0, 3, None),
            (1040, 0, 1, Some(false)),
            (1105, 5, 1, Some(false)),
            (1217, 6, 3, None),
        ],
    ),
    Solution {
        level: 64,
        plan: Plan::Rated(
            &[(0, 40)],
            &[
                (6, 0, 8, None),
                (170, 4, 8, None),
                (334, 8, 8, None),
                (498, 12, 8, None),
                (662, 16, 8, None),
                (1205, 1, 2, None),
                (1403, 5, 2, None),
                (1595, 2, 6, None),
                (1599, 9, 2, None),
                (2060, 3, 3, None),
            ],
        ),
        saves: 0,
    },
    at(
        65,
        &[
            (510, 0, 1, Some(false)),
            (510, 2, 1, Some(false)),
            (514, 4, 1, Some(false)),
            (518, 6, 1, Some(false)),
            (574, 5, 1, Some(true)),
            (578, 3, 1, Some(true)),
            (578, 7, 1, Some(true)),
            (582, 14, 1, Some(true)),
            (596, 1, 1, Some(true)),
            (650, 9, 1, Some(false)),
        ],
    ),
    Solution {
        level: 69,
        plan: Plan::Rated(
            &[(300, 64)],
            &[
                (46, 0, 0, None),
                (145, 1, 1, Some(false)),
                (300, 2, 3, None),
                (960, 2, 1, Some(true)),
                (1080, 4, 0, None),
                (2120, 8, 3, None),
            ],
        ),
        saves: 0,
    },
    // Mayhem 11 "Across the Network" by the owner's route, the one the
    // original's walkthrough video plays (saving 75 there too).
    at(
        70,
        &[
            (252, 1, 0, None),
            (538, 0, 1, Some(false)),
            (593, 6, 1, Some(false)),
            (760, 5, 4, None),
            (800, 6, 2, None),
            (1090, 2, 3, None),
            (1245, 12, 4, None),
        ],
    ),
    at(
        71,
        &[
            (90, 1, 7, None),
            (170, 2, 7, None),
            (230, 0, 1, Some(false)),
            (478, 1, 1, Some(true)),
            (593, 3, 1, Some(true)),
            (670, 2, 5, None),
            (995, 4, 1, Some(false)),
            (1108, 5, 1, Some(false)),
            (1415, 6, 1, Some(false)),
            (1503, 7, 3, None),
            (1835, 8, 1, Some(false)),
            (2105, 9, 1, Some(false)),
            (2275, 10, 1, Some(false)),
            (2465, 11, 6, None),
        ],
    ),
    // By the published walkthroughs; Mayhem 8 "Five Towers" with one more
    // bridge than the walkthrough's.
    at(
        67,
        &[
            (345, 1, 1, Some(true)),
            (482, 3, 1, Some(false)),
            (686, 5, 1, Some(true)),
            (780, 7, 3, None),
            (940, 7, 3, None),
            (1098, 7, 3, None),
            (1236, 7, 3, None),
            (1402, 7, 3, None),
            (1431, 7, 1, Some(false)),
            (3291, 39, 5, None),
            (3660, 25, 1, Some(true)),
        ],
    ),
    // Mayhem 9 "Over the Top", by the published walkthrough.
    at(
        68,
        &[
            (1, 0, 7, None),
            (67, 1, 7, None),
            (380, 0, 1, Some(false)),
            (760, 1, 3, None),
            (1855, 1, 6, None),
        ],
    ),
    Solution {
        level: 76,
        plan: Plan::Rated(
            &[(280, 49)],
            &[
                (70, 0, 6, None),
                (115, 0, 4, None),
                (274, 1, 2, None),
                (1230, 2, 4, None),
            ],
        ),
        saves: 0,
    },
    // Tricky levels by the published walkthroughs.
    at(
        28,
        &[
            (249, 0, 1, Some(false)),
            (335, 1, 1, Some(true)),
            (479, 2, 1, Some(false)),
            (562, 3, 1, Some(true)),
            (677, 4, 1, Some(true)),
            (921, 5, 4, None),
            (1253, 9, 1, Some(false)),
            (1319, 10, 4, None),
            (1559, 10, 3, None),
        ],
    ),
    at(
        29,
        &[
            (24, 0, 6, None),
            (491, 3, 1, Some(true)),
            (735, 0, 1, Some(false)),
            (892, 4, 1, Some(true)),
            (1199, 5, 4, None),
            (1328, 5, 1, Some(false)),
            (1472, 6, 3, None),
            (1754, 6, 1, Some(true)),
            (1792, 7, 3, None),
            (1951, 7, 3, None),
        ],
    ),
    at(
        30,
        &[
            (87, 0, 1, Some(false)),
            (355, 1, 1, Some(false)),
            (623, 2, 1, Some(false)),
            (795, 3, 1, Some(false)),
            (1127, 4, 1, Some(true)),
            (1267, 5, 1, Some(true)),
            (1568, 6, 1, Some(true)),
            (1679, 7, 4, None),
            (1804, 7, 1, Some(true)),
            (2105, 8, 1, Some(true)),
            (2200, 9, 3, None),
            (2307, 9, 1, Some(true)),
            (2418, 10, 1, Some(true)),
            (2591, 11, 1, Some(true)),
            (2682, 12, 4, None),
            (2763, 12, 1, Some(true)),
            (2904, 13, 1, Some(true)),
            (3045, 14, 1, Some(true)),
            (3313, 15, 1, Some(true)),
            (3421, 16, 1, Some(true)),
            (3657, 17, 1, Some(false)),
            (3829, 18, 1, Some(false)),
        ],
    ),
    // Tricky 12 "Dot To Dot" by the published walkthrough.
    at(
        31,
        &[
            (98, 0, 1, Some(true)),
            (188, 1, 3, None),
            (347, 1, 3, None),
            (495, 1, 1, Some(false)),
            (584, 3, 3, None),
            (743, 3, 3, None),
            (891, 3, 1, Some(false)),
            (1009, 5, 3, None),
            (1170, 5, 3, None),
            (1318, 5, 1, Some(true)),
            (1436, 7, 3, None),
            (1598, 7, 3, None),
            (1790, 7, 1, Some(true)),
            (1868, 9, 3, None),
            (2027, 9, 3, None),
            (2175, 9, 1, Some(false)),
            (2292, 11, 3, None),
            (2454, 11, 3, None),
            (2602, 11, 1, Some(false)),
            (2721, 13, 1, Some(false)),
            (2839, 14, 1, Some(true)),
            (2957, 15, 1, Some(true)),
            (3074, 16, 1, Some(false)),
            (3164, 17, 3, None),
            (3323, 17, 3, None),
            (3471, 17, 3, None),
            (3645, 17, 3, None),
            (3793, 17, 1, Some(false)),
            (3899, 20, 1, Some(false)),
            (4012, 21, 1, Some(true)),
            (4106, 22, 3, None),
            (4265, 22, 3, None),
            (4457, 22, 3, None),
            (4619, 22, 3, None),
            (4767, 22, 1, Some(true)),
            (4873, 25, 1, Some(true)),
            (4990, 26, 1, Some(true)),
            (5140, 27, 1, Some(true)),
            (5259, 28, 1, Some(true)),
            (5348, 29, 3, None),
            (5507, 29, 3, None),
            (5655, 29, 1, Some(false)),
            (5744, 31, 3, None),
            (5903, 31, 3, None),
            (6051, 31, 1, Some(false)),
            (6140, 33, 3, None),
            (6299, 33, 3, None),
            (6487, 33, 3, None),
            (6653, 33, 3, None),
        ],
    ),
    at(
        33,
        &[
            (408, 0, 1, Some(true)),
            (478, 1, 4, None),
            (618, 1, 1, Some(true)),
            (882, 3, 1, Some(true)),
            (1061, 4, 1, Some(false)),
            (1247, 5, 1, Some(true)),
            (1643, 6, 3, None),
            (1834, 7, 1, Some(false)),
            (2346, 8, 5, None),
        ],
    ),
    // Taxing levels by the published walkthroughs; Taxing 11 "Chaos Maze" stops its digger
    // with a basher where the walkthrough gives a turner.
    at(
        43,
        &[
            (150, 2, 7, None),
            (151, 2, 8, None),
            (312, 2, 3, None),
            (718, 2, 3, None),
            (1188, 2, 3, None),
            (1388, 5, 4, None),
            (1748, 5, 3, None),
            (2077, 2, 3, None),
            (2240, 2, 4, None),
            (2312, 0, 4, None),
            (2362, 2, 1, Some(false)),
            (2384, 1, 4, None),
        ],
    ),
    at(
        45,
        &[
            (260, 0, 1, Some(false)),
            (1630, 1, 1, Some(true)),
            (1690, 3, 7, None),
            (1691, 3, 8, None),
            (1810, 5, 7, None),
            (1811, 5, 8, None),
            (2005, 3, 0, None),
            (2150, 5, 3, None),
            (2380, 3, 2, None),
            (2388, 7, 6, None),
        ],
    ),
    Solution {
        level: 46,
        plan: Plan::Rated(
            &[(250, 79)],
            &[
                (140, 0, 3, None),
                (380, 0, 1, Some(false)),
                (712, 4, 1, Some(true)),
                (867, 16, 1, Some(true)),
                (938, 20, 1, Some(true)),
                (1027, 24, 6, None),
                (1656, 2, 4, None),
                (1738, 3, 4, None),
                (2168, 1, 4, None),
                (3150, 2, 1, Some(true)),
            ],
        ),
        saves: 0,
    },
    at(
        50,
        &[
            (5, 0, 7, None),
            (193, 0, 6, None),
            (255, 0, 4, None),
            (296, 0, 1, Some(true)),
            (450, 3, 1, Some(true)),
            (655, 4, 1, Some(false)),
            (770, 5, 1, Some(true)),
            (1075, 6, 1, Some(true)),
            (1478, 7, 1, Some(true)),
            (1880, 8, 1, Some(true)),
            (2030, 9, 1, Some(false)),
            (2335, 10, 1, Some(true)),
        ],
    ),
    // Re-solved after turners were found to turn only walkers going their
    // way. Tricky 7 "Follow the Yellow Brick Road" pens the early lemmings
    // first; Taxing 8 "Rescue Team", Mayhem 13 "The Catacombs" (past the
    // trap while it is busy) and Mayhem 19 "Tuther Circus Level" by routes
    // of our own.
    at(
        26,
        &[
            (56, 0, 3, None),
            (116, 1, 1, Some(true)),
            (228, 0, 3, None),
            (274, 2, 0, None),
            (386, 0, 3, None),
            (430, 3, 0, None),
            (544, 0, 3, None),
            (603, 0, 6, None),
            (650, 6, 1, Some(true)),
            (997, 0, 4, None),
            (1110, 0, 6, None),
            (1334, 0, 1, Some(true)),
            (1345, 7, 4, None),
            (1455, 7, 1, Some(true)),
            (1548, 4, 4, None),
            (1600, 4, 6, None),
            (1822, 4, 4, None),
            (1872, 4, 1, Some(false)),
            (1945, 5, 4, None),
            (2065, 5, 1, Some(false)),
            (2190, 8, 4, None),
            (2370, 8, 1, Some(false)),
            (2460, 9, 4, None),
            (2645, 9, 1, Some(true)),
            (2730, 10, 4, None),
        ],
    ),
    at(
        47,
        &[
            (337, 1, 1, Some(true)),
            (573, 5, 1, Some(false)),
            (895, 17, 1, Some(false)),
            (1000, 9, 1, Some(false)),
            (1228, 21, 1, Some(false)),
            (1440, 13, 4, None),
            (1443, 25, 4, None),
            (1702, 25, 1, Some(true)),
            (2198, 29, 4, None),
            (2254, 30, 1, Some(true)),
            (2505, 33, 4, None),
            (2716, 34, 1, Some(false)),
            (2862, 33, 1, Some(false)),
        ],
    ),
    at(
        53,
        &[
            (90, 0, 1, Some(false)),
            (150, 1, 1, Some(false)),
            (375, 2, 1, Some(false)),
            (930, 4, 1, Some(false)),
            (1075, 6, 1, Some(false)),
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
            (1765, 6, 1, Some(false)),
            (1915, 8, 1, Some(false)),
            (2040, 9, 1, Some(false)),
        ],
    ),
    Solution {
        level: 72,
        plan: Plan::Rated(
            &[(850, 49)],
            &[
                (96, 0, 1, Some(true)),
                (211, 1, 1, Some(false)),
                (326, 2, 1, Some(false)),
                (441, 3, 1, Some(true)),
                (716, 4, 1, Some(true)),
                (831, 5, 1, Some(true)),
                (1042, 6, 1, Some(false)),
                (1157, 7, 1, Some(false)),
                (1432, 8, 1, Some(true)),
                (1547, 9, 1, Some(true)),
                (1662, 10, 1, Some(false)),
                (1937, 11, 1, Some(false)),
                (2052, 12, 1, Some(true)),
                (2167, 13, 1, Some(true)),
                (2666, 14, 1, Some(true)),
                (2909, 15, 1, Some(true)),
                (3152, 16, 1, Some(false)),
                (3306, 18, 1, Some(true)),
                (3365, 19, 1, Some(true)),
                (3527, 20, 1, Some(false)),
                (3593, 21, 1, Some(true)),
                (3659, 22, 1, Some(true)),
                (3789, 23, 1, Some(true)),
                (3855, 24, 1, Some(false)),
                (3953, 25, 1, Some(false)),
            ],
        ),
        saves: 0,
    },
    Solution {
        level: 78,
        plan: Plan::Rated(
            &[(1150, 49)],
            &[
                (5, 0, 7, None),
                (60, 1, 7, None),
                (180, 0, 3, None),
                (544, 1, 4, None),
                (1200, 1, 4, None),
                (1500, 4, 4, None),
                (1650, 0, 2, None),
            ],
        ),
        saves: 0,
    },
    // Tricky 6 "Lemmhanger" by the walkthrough; Taxing 19 "Lemmtris" by
    // xeye's route, the exit tunnel mined in two goes.
    at(
        25,
        &[
            (10, 0, 3, None),
            (200, 1, 1, Some(true)),
            (545, 2, 1, Some(false)),
            (705, 3, 0, None),
            (860, 4, 1, Some(true)),
            (1229, 5, 1, Some(false)),
            (1357, 6, 3, None),
            (1490, 6, 1, Some(true)),
            (1630, 7, 4, None),
            (1955, 7, 0, None),
        ],
    ),
    at(
        58,
        &[
            (2, 0, 8, None),
            (68, 1, 3, None),
            (424, 0, 5, None),
            (1043, 0, 3, None),
            (1200, 1, 2, None),
            (1205, 0, 5, None),
            (1231, 0, 5, None),
        ],
    ),
    // Taxing 9 "Which Trampoline ?" by a route of our own, east of the
    // walkthrough's, into the exit's east door; hatch lemmings land past the
    // centre of the first turner's cell and are turned where they land.
    at(
        48,
        &[
            (9, 0, 1, Some(true)),
            (370, 1, 1, Some(false)),
            (577, 2, 1, Some(false)),
            (708, 3, 1, Some(true)),
            (840, 4, 1, Some(false)),
            (975, 5, 1, Some(true)),
            (1113, 6, 1, Some(true)),
        ],
    ),
    // Taxing 10 "It's the Name of the Game" by a route of our own, found once
    // the trampolines were measured.
    at(
        49,
        &[
            (20, 0, 7, None),
            (132, 0, 4, None),
            (515, 0, 3, None),
            (846, 0, 6, None),
            (990, 0, 2, None),
            (1200, 3, 5, None),
            (1384, 3, 3, None),
            (1440, 3, 4, None),
            (1895, 3, 1, Some(true)),
            (2068, 10, 1, Some(false)),
            (2168, 1, 1, Some(true)),
            (2329, 11, 1, Some(true)),
            (2419, 45, 1, Some(false)),
        ],
    ),
    // Mayhem 14 "Jelly Belly Islands" by a route of our own, after the
    // walkthroughs' climbers, pillar bridges and splitter builders.
    at(
        73,
        &[
            (100, 1, 7, None),
            (168, 0, 1, Some(false)),
            (440, 1, 3, None),
            (530, 2, 1, Some(false)),
            (635, 3, 1, Some(true)),
            (640, 4, 7, None),
            (656, 1, 1, Some(true)),
            (922, 4, 3, None),
            (1010, 5, 1, Some(true)),
            (1138, 4, 1, Some(false)),
            (1351, 6, 3, None),
            (1432, 7, 3, None),
        ],
    ),
];

pub fn solution(level: u32) -> Option<&'static Solution> {
    SOLUTIONS.iter().find(|s| s.level == level)
}

/// Plays a solution through and returns the finished simulation, whose
/// `log` replays it.
pub fn play(s: &Solution, level: &Level, blocks: &BlockSet) -> Simulation {
    match s.plan {
        Plan::At(steps) => play_steps(&[], steps, level, blocks),
        Plan::Rated(rates, steps) => play_steps(rates, steps, level, blocks),
        Plan::Each(id) => {
            let mut sim = Simulation::new(level, blocks);
            while !sim.finished() {
                sim.step();
                if let Some(skill) = Skill::from_id(id) {
                    for i in 0..sim.lemmings.len() {
                        sim.assign(i, skill);
                    }
                }
            }
            sim
        }
    }
}

/// Plays release-rate changes and assignments through as [`Plan::Rated`]
/// does and returns the finished simulation, whose `log` replays it.
pub fn play_steps(
    rates: &[RateStep],
    steps: &[Step],
    level: &Level,
    blocks: &BlockSet,
) -> Simulation {
    let mut sim = Simulation::new(level, blocks);
    while !sim.finished() {
        sim.step();
        let now = sim.tick;
        let due = |t: u64| t + crate::HATCH_OPEN_TICKS as u64 == now;
        for &(_, change) in rates.iter().filter(|(t, _)| due(*t)) {
            sim.adjust_release_rate(change);
        }
        for &step in steps.iter().filter(|(t, ..)| due(*t)) {
            apply_step(&mut sim, step);
        }
    }
    sim
}

/// Gives out `step`'s skill now, whatever its tick, and returns whether it
/// was taken (a rejected assignment leaves no command in the log). With a
/// side, a lemming that is out gets a turner pointing to that side of its
/// heading; otherwise the skill is assigned as is.
pub fn apply_step(sim: &mut Simulation, (_, i, id, side): Step) -> bool {
    match (side, sim.lemmings.get(i).map(|l| l.dir)) {
        (Some(cw), Some(d)) => {
            sim.assign_turner(i, if cw { d.clockwise() } else { d.anticlockwise() })
        }
        _ => Skill::from_id(id).is_some_and(|skill| sim.assign(i, skill)),
    }
}

/// Parses a step written `TICK:LEMMING:SKILL`, with a fourth part `cw` or
/// `acw` for a turner's side; any other fourth part, and any after it, are
/// ignored.
pub fn parse_step(s: &str) -> Option<Step> {
    let mut p = s.split(':');
    let t = p.next()?.parse().ok()?;
    let i = p.next()?.parse().ok()?;
    let id = p.next()?.parse().ok()?;
    let side = match p.next() {
        Some("cw") => Some(true),
        Some("acw") => Some(false),
        _ => None,
    };
    Some((t, i, id, side))
}

/// Parses a release-rate change written `rTICK:CHANGE`.
pub fn parse_rate_step(s: &str) -> Option<RateStep> {
    let (t, c) = s.strip_prefix('r')?.split_once(':')?;
    Some((t.parse().ok()?, c.parse().ok()?))
}

/// The commands of level `n`'s demo, if it has one.
pub fn demo(n: u32, level: &Level, blocks: &BlockSet) -> Option<Vec<(u64, Command)>> {
    solution(n).map(|s| play(s, level, blocks).log)
}
