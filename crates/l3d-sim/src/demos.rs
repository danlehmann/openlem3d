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
    at(91, &[(576, 0, 4, None)]),
    at(92, &[(100, 0, 3, None), (265, 0, 3, None)]),
    at(93, &[(122, 0, 4, None)]),
    at(94, &[(182, 0, 1, Some(false))]),
    at(95, &[(239, 0, 1, Some(true))]),
    at(96, &[(171, 0, 1, Some(false))]),
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
            (225, 0, 2, None),
            (436, 1, 4, None),
        ],
    ),
    // Found by play-testing in the simulation; each looks like the designed
    // route.
    at(
        2,
        &[(143, 0, 3, None), (752, 0, 3, None), (923, 0, 3, None)],
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
            (626, 2, 2, None),
            (900, 3, 2, None),
            (1360, 4, 2, None),
        ],
    ),
    at(
        7,
        &[
            (476, 0, 1, Some(false)),
            (621, 1, 3, None),
            (785, 1, 3, None),
            (846, 3, 1, Some(false)),
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
            (132, 0, 1, Some(true)),
            (272, 1, 5, None),
            (652, 2, 1, Some(false)),
            (760, 3, 3, None),
            (1204, 4, 1, Some(false)),
            (1460, 5, 3, None),
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
            (88, 3, 4, None),
            (120, 2, 4, None),
            (152, 1, 4, None),
            (184, 0, 4, None),
            (330, 3, 3, None),
            (362, 2, 3, None),
            (394, 1, 3, None),
            (426, 0, 3, None),
            (613, 3, 2, None),
            (645, 2, 2, None),
            (677, 1, 2, None),
            (709, 0, 2, None),
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
    // Mayhem 1 "The Five Arches", after the published route (saves 78 of
    // 78); solvable once a digger turned builder walked to its cell's edge
    // first, as in the original.
    Solution {
        level: 60,
        plan: Plan::Rated(
            &[(600, 49)],
            &[
                (39, 0, 7, None),
                (385, 0, 6, None),
                (568, 1, 7, None),
                (619, 2, 7, None),
                (655, 0, 3, None),
                (697, 2, 6, None),
                (851, 2, 2, None),
                (1024, 0, 2, None),
                (1277, 1, 6, None),
            ],
        ),
        saves: 0,
    },
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
            (960, 2, 3, None),
            (1975, 2, 3, None),
            (2079, 2, 4, None),
        ],
    ),
    at(
        41,
        &[
            (58, 0, 1, Some(false)),
            (126, 4, 7, None),
            (147, 2, 1, Some(true)),
            (177, 6, 7, None),
            (228, 8, 7, None),
            (250, 0, 2, None),
            (359, 1, 0, None),
            (376, 4, 1, Some(true)),
            (450, 6, 1, Some(true)),
            (520, 6, 2, None),
            (524, 8, 4, None),
            (560, 10, 0, None),
            (1160, 12, 4, None),
            (1802, 12, 1, Some(true)),
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
            (1001, 8, 3, None),
            (1133, 7, 1, Some(false)),
            (1165, 8, 3, None),
            (1282, 10, 1, Some(false)),
            (1322, 9, 1, Some(false)),
            (1617, 14, 1, Some(true)),
        ],
    ),
    at(
        27,
        &[
            (360, 0, 2, None),
            (451, 1, 6, None),
            (917, 3, 4, None),
            (1034, 2, 0, None),
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
    // Fun 2 by a route of our own (saves 76 of 70).
    Solution {
        level: 1,
        plan: Plan::Rated(
            &[(187, 99)],
            &[
                (12, 0, 8, None),
                (56, 2, 8, None),
                (77, 3, 8, None),
                (119, 5, 8, None),
                (140, 6, 8, None),
                (161, 7, 8, None),
                (182, 8, 8, None),
                (251, 1, 6, None),
                (348, 9, 8, None),
                (348, 79, 8, None),
                (532, 4, 0, None),
                (608, 0, 6, None),
            ],
        ),
        saves: 0,
    },
    at(
        8,
        &[
            (60, 0, 1, Some(true)),
            (206, 1, 1, Some(false)),
            (322, 2, 1, Some(false)),
            (469, 3, 1, Some(true)),
            (616, 4, 1, Some(false)),
            (709, 5, 3, None),
            (878, 6, 1, Some(false)),
            (992, 7, 1, Some(true)),
        ],
    ),
    at(
        9,
        &[
            (50, 0, 3, None),
            (216, 0, 3, None),
            (409, 0, 1, Some(true)),
            (1351, 2, 1, Some(true)),
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
                (135, 0, 3, None),
                (385, 0, 1, Some(false)),
                (463, 1, 1, Some(true)),
                (671, 2, 1, Some(true)),
                (816, 3, 1, Some(true)),
                (1290, 4, 6, None),
            ],
        ),
        saves: 0,
    },
    // Fun 17 "Hole in Ten", the owner's route: a blocker (and floater) on
    // the wedge atop the left ramp; the first walker back up that ramp bashes
    // through the wedge, freeing the blocker, which floats down, is turned
    // at the stone block and bashes through the foot of the right ramp.
    at(
        16,
        &[
            (112, 0, 0, None),
            (113, 0, 8, None),
            (860, 1, 4, None),
            (1530, 0, 4, None),
        ],
    ),
    // The published route (mining round the ring), the release rate raised
    // late so the last lemmings get round in time (saves 58 of 50).
    Solution {
        level: 17,
        plan: Plan::Rated(
            &[(3000, 20)],
            &[
                (15, 0, 5, None),
                (375, 0, 1, Some(true)),
                (467, 3, 5, None),
                (834, 3, 1, Some(true)),
                (927, 6, 5, None),
                (1294, 6, 1, Some(true)),
                (1387, 9, 5, None),
                (1753, 9, 1, Some(true)),
                (1847, 12, 5, None),
                (2213, 12, 1, Some(true)),
                (2307, 15, 5, None),
                (2674, 15, 1, Some(true)),
                (2767, 18, 5, None),
                (3134, 18, 1, Some(true)),
                (3227, 21, 5, None),
                (3425, 21, 1, Some(true)),
            ],
        ),
        saves: 0,
    },
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
            (746, 3, 3, None),
            (966, 3, 1, Some(true)),
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
            (240, 0, 1, Some(true)),
            (335, 1, 5, None),
            (535, 1, 0, None),
            (755, 2, 1, Some(false)),
            (902, 3, 3, None),
            (1068, 4, 6, None),
            (1272, 4, 1, Some(false)),
            (1480, 7, 1, Some(false)),
            (1576, 8, 3, None),
        ],
    ),
    at(
        35,
        &[
            (236, 0, 1, Some(false)),
            (455, 1, 3, None),
            (666, 1, 1, Some(false)),
            (825, 2, 4, None),
            (1245, 2, 4, None),
            (1655, 2, 1, Some(true)),
        ],
    ),
    at(
        36,
        &[
            (252, 0, 1, Some(false)),
            (418, 1, 1, Some(true)),
            (590, 3, 7, None),
            (642, 3, 6, None),
            (710, 3, 2, None),
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
            (592, 3, 3, None),
            (880, 3, 1, Some(true)),
            (1060, 5, 1, Some(false)),
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
            (887, 79, 3, None),
            (1080, 79, 3, None),
            (1281, 79, 3, None),
            (1480, 79, 3, None),
            (1806, 79, 3, None),
            (2280, 79, 4, None),
            (2420, 79, 7, None),
            (2734, 79, 3, None),
            (3200, 79, 4, None),
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
            (502, 0, 4, None),
            (775, 1, 3, None),
            (952, 0, 5, None),
            (985, 2, 6, None),
        ],
    ),
    // Taxing 16 "The Arena" by xeye's route: three blocks blown up, then
    // the walkers turned at the inside of the L-shaped sandstone block and
    // a bridge built from the top of the small one.
    at(
        55,
        &[
            (110, 0, 2, None),
            (1260, 1, 2, None),
            (2160, 2, 2, None),
            (2603, 3, 1, Some(true)),
            (2721, 4, 3, None),
        ],
    ),
    // Taxing 17 by the published walkthrough.
    Solution {
        level: 56,
        plan: Plan::Rated(&[(0, 49)], &[(359, 0, 2, None)]),
        saves: 0,
    },
    Solution {
        level: 57,
        plan: Plan::Rated(
            &[],
            &[
                (68, 0, 1, Some(true)),
                (119, 1, 0, None),
                (351, 2, 1, Some(false)),
                (542, 4, 1, Some(false)),
                (797, 6, 1, Some(true)),
                (956, 8, 1, Some(true)),
                (1179, 10, 1, Some(false)),
                (1338, 12, 1, Some(false)),
                (1361, 78, 2, None),
                (1529, 14, 1, Some(true)),
                (1656, 16, 1, Some(true)),
                (1746, 76, 2, None),
                (1815, 18, 1, Some(false)),
                (1942, 20, 1, Some(false)),
                (2035, 74, 2, None),
                (2069, 22, 1, Some(true)),
                (2164, 24, 1, Some(true)),
                (2259, 26, 1, Some(false)),
                (2324, 72, 2, None),
                (2354, 28, 1, Some(false)),
                (2385, 30, 2, None),
                (2448, 32, 1, Some(true)),
                (2517, 70, 2, None),
                (2710, 68, 2, None),
                (2807, 66, 2, None),
            ],
        ),
        saves: 0,
    },
    // Taxing 20 "Death Slide", partly by the published walkthrough.
    at(
        59,
        &[
            (16, 0, 7, None),
            (97, 1, 7, None),
            (400, 0, 1, Some(true)),
            (515, 1, 3, None),
            (650, 1, 5, None),
            (715, 2, 3, None),
            (1290, 2, 1, Some(true)),
            (1485, 3, 0, None),
            (1615, 4, 3, None),
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
            (518, 2, 3, None),
            (569, 5, 1, Some(false)),
            (708, 7, 3, None),
            (816, 10, 1, Some(true)),
            (860, 7, 3, None),
            (999, 15, 1, Some(true)),
            (1022, 7, 3, None),
            (1078, 22, 1, Some(false)),
            (1214, 20, 1, Some(false)),
            (1278, 25, 3, None),
            (1930, 1, 5, None),
            (2530, 4, 5, None),
        ],
    ),
    at(
        63,
        &[
            (283, 0, 8, None),
            (288, 0, 7, None),
            (449, 1, 4, None),
            (649, 0, 4, None),
            (871, 0, 3, None),
            (1059, 0, 1, Some(false)),
            (1124, 5, 1, Some(false)),
            (1236, 6, 3, None),
        ],
    ),
    Solution {
        level: 64,
        plan: Plan::Rated(
            &[(0, 40)],
            &[
                (6, 0, 8, None),
                (47, 4, 8, None),
                (88, 8, 8, None),
                (129, 12, 8, None),
                (170, 16, 8, None),
                (1164, 1, 2, None),
                (1239, 5, 2, None),
                (1312, 9, 2, None),
                (1513, 2, 6, None),
                (1937, 3, 3, None),
            ],
        ),
        saves: 0,
    },
    at(
        65,
        &[
            (508, 1, 1, Some(false)),
            (512, 3, 1, Some(false)),
            (516, 5, 1, Some(false)),
            (524, 7, 1, Some(false)),
            (564, 0, 1, Some(true)),
            (568, 2, 1, Some(true)),
            (572, 9, 1, Some(true)),
            (580, 4, 1, Some(true)),
            (606, 6, 1, Some(true)),
            (660, 14, 1, Some(false)),
        ],
    ),
    Solution {
        level: 69,
        plan: Plan::Rated(
            &[(300, 64)],
            &[
                (46, 0, 0, None),
                (145, 1, 1, Some(false)),
                (314, 2, 3, None),
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
            (225, 0, 0, None),
            (573, 1, 1, Some(false)),
            (659, 8, 1, Some(false)),
            (733, 4, 4, None),
            (804, 8, 2, None),
            (1125, 3, 3, None),
            (1249, 12, 4, None),
        ],
    ),
    // Mayhem 12 "Raiders of the Lost Lemming" by the Lemmings Wiki strategy
    // (its flame-blowers rule out the published route as transcribed).
    at(
        71,
        &[
            (97, 1, 7, None),
            (177, 2, 7, None),
            (237, 0, 1, Some(false)),
            (485, 1, 1, Some(true)),
            (600, 3, 1, Some(true)),
            (660, 2, 5, None),
            (1247, 7, 1, Some(false)),
            (1360, 8, 1, Some(false)),
            (1665, 9, 1, Some(false)),
            (1747, 10, 3, None),
            (1944, 10, 1, Some(false)),
            (1987, 11, 6, None),
            (2101, 12, 1, Some(true)),
            (2137, 11, 1, Some(true)),
        ],
    ),
    // Mayhem 8 "Five Towers" by a route of our own (saves 30 of 30): with
    // both hatches dropping at once, the lemmings right behind each builder
    // walk off its unfinished staircase, so the first builder is made a
    // turner on its last brick and the lemming behind it builds on from there.
    at(
        67,
        &[
            (294, 1, 1, Some(true)),
            (415, 3, 3, None),
            (553, 3, 1, Some(true)),
            (558, 5, 3, None),
            (713, 5, 3, None),
            (878, 5, 3, None),
            (1046, 13, 1, Some(true)),
            (1057, 5, 3, None),
            (1219, 5, 3, None),
            (1430, 0, 2, None),
            (1535, 2, 1, Some(false)),
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
    // Mayhem 15 "Hole in One, Two, Three!", by the published walkthrough.
    at(
        74,
        &[
            (185, 1, 4, None),
            (335, 0, 3, None),
            (622, 2, 3, None),
            (770, 0, 4, None),
            (830, 2, 6, None),
            (1090, 0, 4, None),
            (1100, 2, 3, None),
            (1267, 0, 4, None),
        ],
    ),
    // Mayhem 16 "Fun Fair", after the published route with changes of our
    // own (saves 33 of 25).
    at(
        75,
        &[
            (96, 1, 7, None),
            (177, 2, 7, None),
            (218, 0, 3, None),
            (258, 3, 7, None),
            (339, 4, 7, None),
            (420, 5, 7, None),
            (426, 0, 3, None),
            (501, 6, 7, None),
            (623, 0, 1, Some(true)),
            (862, 1, 1, Some(false)),
            (1060, 7, 1, Some(true)),
            (1121, 3, 6, None),
            (1218, 2, 1, Some(false)),
            (1487, 4, 0, None),
            (1654, 6, 5, None),
            (1993, 15, 3, None),
            (2186, 15, 1, Some(false)),
        ],
    ),
    Solution {
        level: 76,
        plan: Plan::Rated(
            &[(280, 49)],
            &[
                (69, 0, 6, None),
                (127, 0, 4, None),
                (274, 1, 2, None),
                (1256, 2, 4, None),
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
            (34, 0, 6, None),
            (501, 3, 1, Some(true)),
            (745, 0, 1, Some(false)),
            (902, 4, 1, Some(true)),
            (1209, 5, 4, None),
            (1338, 5, 1, Some(false)),
            (1482, 6, 3, None),
            (1764, 6, 1, Some(true)),
            (1802, 7, 3, None),
            (1961, 7, 3, None),
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
            (2212, 9, 3, None),
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
            (109, 0, 1, Some(true)),
            (199, 1, 3, None),
            (358, 1, 3, None),
            (540, 1, 1, Some(false)),
            (595, 3, 3, None),
            (754, 3, 3, None),
            (935, 3, 1, Some(false)),
            (1020, 5, 3, None),
            (1181, 5, 3, None),
            (1360, 5, 1, Some(true)),
            (1447, 7, 3, None),
            (1609, 7, 3, None),
            (1835, 7, 1, Some(true)),
            (1874, 9, 3, None),
            (2040, 9, 3, None),
            (2225, 9, 1, Some(false)),
            (2302, 11, 3, None),
            (2468, 11, 3, None),
            (2650, 11, 1, Some(false)),
            (2735, 13, 1, Some(false)),
            (2853, 14, 1, Some(true)),
            (2971, 15, 1, Some(true)),
            (3088, 16, 1, Some(false)),
            (3172, 17, 3, None),
            (3338, 17, 3, None),
            (3532, 17, 3, None),
            (3645, 20, 3, None),
            (3701, 17, 3, None),
            (3884, 17, 1, Some(false)),
            (3969, 20, 1, Some(false)),
            (4013, 21, 1, Some(true)),
            (4112, 22, 3, None),
            (4278, 22, 3, None),
            (4511, 22, 3, None),
            (4620, 25, 3, None),
            (4676, 22, 3, None),
            (4852, 22, 1, Some(true)),
            (4944, 25, 1, Some(true)),
            (4991, 26, 1, Some(true)),
            (5140, 27, 1, Some(true)),
            (5259, 28, 1, Some(true)),
            (5354, 29, 3, None),
            (5520, 29, 3, None),
            (5700, 29, 1, Some(false)),
            (5750, 31, 3, None),
            (5916, 31, 3, None),
            (6097, 31, 1, Some(false)),
            (6146, 33, 3, None),
            (6312, 33, 3, None),
            (6540, 33, 3, None),
            (6706, 33, 3, None),
        ],
    ),
    at(
        33,
        &[
            (408, 0, 1, Some(true)),
            (478, 1, 4, None),
            (634, 1, 1, Some(true)),
            (898, 3, 1, Some(true)),
            (1077, 4, 1, Some(false)),
            (1263, 5, 1, Some(true)),
            (1659, 6, 3, None),
            (1850, 7, 1, Some(false)),
            (2362, 8, 5, None),
        ],
    ),
    // Taxing levels by the published walkthroughs; Taxing 11 "Chaos Maze" stops its digger
    // with a basher where the walkthrough gives a turner.
    at(
        43,
        &[
            (8, 2, 7, None),
            (9, 2, 8, None),
            (170, 2, 3, None),
            (580, 2, 3, None),
            (1070, 2, 3, None),
            (1190, 5, 4, None),
            (1568, 2, 3, None),
            (2056, 2, 3, None),
            (2250, 2, 4, None),
            (2312, 0, 4, None),
            (2313, 1, 4, None),
            (2370, 2, 1, Some(false)),
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
                (410, 0, 1, Some(false)),
                (499, 4, 1, Some(true)),
                (873, 16, 1, Some(true)),
                (938, 20, 1, Some(true)),
                (1021, 24, 6, None),
                (1502, 2, 4, None),
                (1719, 3, 4, None),
                (2093, 1, 4, None),
                (3018, 2, 1, Some(true)),
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
            (87, 0, 3, None),
            (125, 1, 1, Some(true)),
            (259, 0, 3, None),
            (273, 2, 0, None),
            (404, 3, 1, Some(true)),
            (417, 0, 3, None),
            (555, 1, 2, None),
            (575, 0, 3, None),
            (655, 0, 6, None),
            (1028, 0, 4, None),
            (1141, 0, 6, None),
            (1365, 0, 1, Some(true)),
            (1376, 7, 4, None),
            (1486, 7, 1, Some(true)),
            (1579, 4, 4, None),
            (1631, 4, 6, None),
            (1853, 4, 4, None),
            (1903, 4, 1, Some(false)),
            (1976, 5, 4, None),
            (2096, 5, 1, Some(false)),
            (2221, 8, 4, None),
            (2401, 8, 1, Some(false)),
            (2491, 9, 4, None),
            (2676, 9, 1, Some(true)),
            (2761, 10, 4, None),
        ],
    ),
    at(
        47,
        &[
            (255, 1, 1, Some(true)),
            (640, 5, 1, Some(false)),
            (885, 9, 1, Some(false)),
            (1042, 53, 1, Some(false)),
            (1090, 13, 4, None),
            (1108, 57, 3, None),
            (1404, 61, 1, Some(false)),
            (1588, 0, 1, Some(false)),
            (1605, 8, 4, None),
            (1740, 57, 4, None),
            (1760, 2, 1, Some(true)),
            (1780, 33, 1, Some(true)),
            (1890, 28, 1, Some(true)),
            (2062, 36, 4, None),
            (2235, 44, 1, Some(false)),
        ],
    ),
    at(
        53,
        &[
            (90, 0, 1, Some(false)),
            (104, 1, 1, Some(false)),
            (329, 2, 1, Some(false)),
            (838, 4, 1, Some(false)),
            (937, 6, 1, Some(false)),
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
                (1210, 1, 4, None),
                (1510, 4, 4, None),
                (1675, 0, 2, None),
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
            (1236, 5, 1, Some(false)),
            (1386, 6, 3, None),
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
            (428, 0, 5, None),
            (1149, 0, 3, None),
            (1292, 1, 2, None),
            (1310, 0, 5, None),
        ],
    ),
    // Taxing 9 "Which Trampoline ?" by a route of our own, east of the
    // walkthrough's, into the exit's east door; hatch lemmings land past the
    // centre of the first turner's cell and are turned where they land.
    at(
        48,
        &[
            (12, 0, 1, Some(true)),
            (373, 1, 1, Some(false)),
            (580, 2, 1, Some(false)),
            (711, 3, 1, Some(true)),
            (843, 4, 1, Some(false)),
            (978, 5, 1, Some(true)),
            (1116, 6, 1, Some(true)),
        ],
    ),
    // Taxing 10 "It's the Name of the Game" by a route of our own, found once
    // the trampolines were measured.
    at(
        49,
        &[
            (21, 0, 7, None),
            (129, 0, 4, None),
            (516, 0, 3, None),
            (860, 0, 6, None),
            (1350, 5, 2, None),
            (1550, 6, 4, None),
            (1750, 6, 5, None),
            (2210, 6, 1, Some(true)),
            (2405, 30, 1, Some(true)),
            (2505, 16, 1, Some(false)),
            (2662, 49, 1, Some(false)),
            (2757, 40, 1, Some(true)),
        ],
    ),
    // Mayhem 14 "Jelly Belly Islands" by a route of our own, after the
    // walkthroughs' climbers, pillar bridges and splitter builders.
    at(
        73,
        &[
            (107, 1, 7, None),
            (175, 0, 1, Some(false)),
            (280, 2, 3, None),
            (447, 1, 3, None),
            (642, 3, 1, Some(true)),
            (672, 1, 1, Some(true)),
            (686, 2, 3, None),
            (802, 5, 1, Some(true)),
        ],
    ),
    // Mayhem 18 "Castle Peralus", after the published strategy with a route
    // of our own for the middle lane.
    at(
        77,
        &[
            (100, 1, 7, None),
            (306, 7, 3, None),
            (420, 7, 4, None),
            (446, 1, 5, None),
            (447, 0, 3, None),
            (447, 2, 3, None),
            (476, 7, 3, None),
            (590, 1, 6, None),
            (606, 0, 3, None),
            (606, 2, 3, None),
            (771, 0, 3, None),
            (771, 2, 3, None),
            (801, 0, 4, None),
            (801, 2, 4, None),
            (854, 7, 3, None),
            (1088, 7, 2, None),
        ],
    ),
    // Mayhem 20 "Final Maze" by a route of our own.
    at(
        79,
        &[
            (112, 0, 1, Some(true)),
            (680, 1, 1, Some(false)),
            (790, 2, 4, None),
            (960, 4, 3, None),
            (1135, 4, 1, Some(true)),
            (1166, 6, 0, None),
            (1206, 7, 0, None),
            (2290, 7, 2, None),
            (2401, 8, 1, Some(false)),
            (2480, 9, 1, Some(true)),
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
