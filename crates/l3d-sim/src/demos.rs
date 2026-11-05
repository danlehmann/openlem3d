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
    at(81, &[(152, 0, 1, Some(true))]),
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
    at(90, &[(408, 0, 1, Some(true))]),
    at(91, &[(576, 0, 4, None)]),
    at(92, &[(100, 0, 3, None), (265, 0, 3, None)]),
    at(93, &[(122, 0, 4, None)]),
    at(94, &[(184, 0, 1, Some(false))]),
    at(95, &[(239, 0, 1, Some(true))]),
    at(96, &[(173, 0, 1, Some(false))]),
    at(97, &[(170, 0, 0, None)]),
    Solution {
        level: 98,
        plan: Plan::At(&[(253, 0, 1, Some(false))]),
        saves: 18,
    },
    at(99, &[(1124, 0, 4, None)]),
    at(0, &[(1411, 0, 1, Some(false))]),
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
            (217, 0, 2, None),
            (436, 1, 4, None),
        ],
    ),
    // Found by play-testing in the simulation; each looks like the designed
    // route.
    at(
        2,
        &[(147, 0, 3, None), (756, 0, 3, None), (927, 0, 3, None)],
    ),
    at(
        3,
        &[
            (225, 0, 6, None),
            (925, 0, 6, None),
            (1615, 0, 1, Some(true)),
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
            (770, 1, 1, Some(true)),
        ],
    ),
    at(
        23,
        &[
            (158, 0, 1, Some(true)),
            (298, 1, 5, None),
            (678, 2, 1, Some(false)),
            (786, 3, 3, None),
            (1230, 4, 1, Some(false)),
            (1486, 5, 3, None),
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
            (8, 0, 7, None),
            (59, 1, 7, None),
            (254, 0, 0, None),
            (348, 1, 4, None),
            (426, 0, 2, None),
            (566, 6, 1, Some(true)),
            (998, 1, 0, None),
            (1056, 4, 7, None),
            (1295, 4, 4, None),
            (1446, 1, 2, None),
            (1520, 13, 1, Some(false)),
            (1890, 15, 7, None),
            (1915, 4, 0, None),
            (2036, 15, 4, None),
            (2114, 4, 2, None),
        ],
    ),
    // Published human solutions (walkthrough sites), timed in the
    // simulation.
    at(21, &[(23, 0, 0, None), (320, 1, 0, None)]),
    at(
        22,
        &[
            (26, 0, 7, None),
            (26, 0, 8, None),
            (107, 1, 7, None),
            (107, 1, 8, None),
            (630, 1, 2, None),
            (724, 0, 5, None),
            (972, 2, 3, None),
            (1987, 2, 3, None),
            (2091, 2, 4, None),
        ],
    ),
    at(
        41,
        &[
            (62, 0, 1, Some(false)),
            (130, 4, 7, None),
            (151, 2, 1, Some(true)),
            (181, 6, 7, None),
            (232, 8, 7, None),
            (232, 0, 2, None),
            (363, 1, 0, None),
            (380, 4, 1, Some(true)),
            (454, 6, 1, Some(true)),
            (481, 6, 2, None),
            (528, 8, 4, None),
            (564, 10, 0, None),
            (1164, 12, 4, None),
            (1806, 12, 1, Some(true)),
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
            (164, 0, 4, None),
            (356, 0, 1, Some(true)),
            (708, 2, 1, Some(true)),
            (860, 3, 3, None),
            (1175, 4, 1, Some(true)),
            (1330, 5, 1, Some(true)),
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
            (62, 0, 1, Some(true)),
            (208, 1, 1, Some(false)),
            (324, 2, 1, Some(false)),
            (471, 3, 1, Some(true)),
            (618, 4, 1, Some(false)),
            (711, 5, 3, None),
            (880, 6, 1, Some(false)),
            (994, 7, 1, Some(true)),
        ],
    ),
    at(
        9,
        &[
            (54, 0, 3, None),
            (220, 0, 3, None),
            (413, 0, 1, Some(true)),
            (1355, 2, 1, Some(true)),
        ],
    ),
    at(
        10,
        &[
            (89, 0, 0, None),
            (364, 1, 1, Some(false)),
            (704, 2, 0, None),
            (1011, 3, 0, None),
            (1256, 4, 0, None),
            (1531, 5, 1, Some(true)),
            (1807, 6, 0, None),
            (2082, 7, 1, Some(false)),
            (2262, 8, 1, Some(true)),
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
            (127, 0, 1, Some(false)),
            (347, 1, 1, Some(false)),
            (559, 2, 1, Some(false)),
            (779, 3, 1, Some(false)),
            (996, 4, 1, Some(false)),
            (1216, 5, 1, Some(false)),
            (1368, 15, 8, None),
            (1447, 6, 1, Some(false)),
            (1459, 16, 8, None),
            (1550, 17, 8, None),
            (1557, 7, 3, None),
            (1641, 18, 8, None),
            (1732, 19, 8, None),
            (1752, 8, 1, Some(false)),
            (1823, 20, 8, None),
            (1914, 21, 8, None),
            (1970, 9, 1, Some(false)),
            (2005, 22, 8, None),
            (2070, 10, 3, None),
            (2096, 23, 8, None),
            (2187, 24, 8, None),
            (2280, 11, 1, Some(false)),
            (2278, 25, 8, None),
            (2369, 26, 8, None),
            (2460, 27, 8, None),
            (2500, 12, 1, Some(false)),
            (2551, 28, 8, None),
            (2642, 29, 8, None),
            (2719, 13, 1, Some(false)),
            (2733, 30, 8, None),
            (2824, 31, 8, None),
            (2873, 14, 1, Some(false)),
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
            (199, 0, 1, Some(false)),
            (404, 1, 1, Some(true)),
            (584, 2, 1, Some(false)),
            (699, 3, 4, None),
            (1184, 4, 1, Some(false)),
        ],
    ),
    Solution {
        level: 15,
        plan: Plan::Rated(
            &[(400, 79)],
            &[
                (137, 0, 3, None),
                (387, 0, 1, Some(false)),
                (465, 1, 1, Some(true)),
                (673, 2, 1, Some(true)),
                (818, 3, 1, Some(true)),
                (1292, 4, 6, None),
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
            (77, 0, 1, Some(false)),
            (228, 1, 6, None),
            (513, 1, 1, Some(false)),
            (683, 3, 1, Some(false)),
            (888, 4, 1, Some(false)),
            (1063, 5, 1, Some(false)),
            (1120, 6, 4, None),
            (1313, 6, 1, Some(true)),
            (1443, 7, 1, Some(true)),
            (1493, 8, 4, None),
            (1740, 8, 1, Some(true)),
            (1928, 9, 1, Some(true)),
            (2108, 10, 1, Some(true)),
        ],
    ),
    at(
        19,
        &[
            (62, 0, 1, Some(false)),
            (332, 1, 1, Some(true)),
            (627, 2, 1, Some(true)),
            (748, 3, 3, None),
            (968, 3, 1, Some(true)),
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
            (238, 0, 1, Some(true)),
            (363, 1, 5, None),
            (550, 1, 0, None),
            (770, 2, 1, Some(false)),
            (917, 3, 3, None),
            (1083, 4, 6, None),
            (1287, 4, 1, Some(false)),
            (1495, 7, 1, Some(false)),
            (1591, 8, 3, None),
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
                (291, 0, 4, None),
                (1186, 0, 7, None),
                (1286, 3, 4, None),
                (1476, 4, 4, None),
                (1705, 0, 6, None),
            ],
        ),
        saves: 0,
    },
    at(
        38,
        &[
            (106, 0, 1, Some(false)),
            (251, 1, 3, None),
            (466, 2, 1, Some(true)),
            (568, 3, 3, None),
            (856, 3, 1, Some(true)),
            (1036, 5, 1, Some(false)),
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
            (18, 0, 7, None),
            (99, 1, 7, None),
            (402, 0, 1, Some(true)),
            (517, 1, 3, None),
            (651, 1, 5, None),
            (717, 2, 3, None),
            (1292, 2, 1, Some(true)),
            (1487, 3, 0, None),
            (1617, 4, 3, None),
        ],
    ),
    // Mayhem levels, play-tested in the simulation; Mayhem 8 "Across the
    // Network" by a route of our own, Mayhem 4 "Driving Range" partly.
    at(
        66,
        &[
            (263, 1, 7, None),
            (353, 0, 1, Some(true)),
            (433, 1, 6, None),
            (541, 2, 3, None),
            (592, 5, 1, Some(false)),
            (731, 7, 3, None),
            (839, 10, 1, Some(true)),
            (878, 7, 3, None),
            (1022, 15, 1, Some(true)),
            (1043, 7, 3, None),
            (1101, 22, 1, Some(false)),
            (1237, 20, 1, Some(false)),
            (1301, 25, 3, None),
            (1953, 1, 5, None),
            (2553, 4, 5, None),
        ],
    ),
    at(
        63,
        &[
            (291, 0, 8, None),
            (296, 0, 7, None),
            (457, 1, 4, None),
            (657, 0, 4, None),
            (879, 0, 3, None),
            (1067, 0, 1, Some(false)),
            (1132, 5, 1, Some(false)),
            (1244, 6, 3, None),
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
            (510, 1, 1, Some(false)),
            (514, 3, 1, Some(false)),
            (518, 5, 1, Some(false)),
            (526, 7, 1, Some(false)),
            (566, 0, 1, Some(true)),
            (570, 2, 1, Some(true)),
            (574, 9, 1, Some(true)),
            (582, 4, 1, Some(true)),
            (608, 6, 1, Some(true)),
            (662, 14, 1, Some(false)),
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
            (227, 0, 0, None),
            (575, 1, 1, Some(false)),
            (661, 8, 1, Some(false)),
            (735, 4, 4, None),
            (806, 8, 2, None),
            (1127, 3, 3, None),
            (1251, 12, 4, None),
        ],
    ),
    // Mayhem 12 "Raiders of the Lost Lemming" by the Lemmings Wiki strategy
    // (its flame-blowers rule out the published route as transcribed).
    at(
        71,
        &[
            (101, 1, 7, None),
            (181, 2, 7, None),
            (241, 0, 1, Some(false)),
            (489, 1, 1, Some(true)),
            (604, 3, 1, Some(true)),
            (664, 2, 5, None),
            (1251, 7, 1, Some(false)),
            (1364, 8, 1, Some(false)),
            (1669, 9, 1, Some(false)),
            (1751, 10, 3, None),
            (1948, 10, 1, Some(false)),
            (1991, 11, 6, None),
            (2105, 12, 1, Some(true)),
            (2141, 11, 1, Some(true)),
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
            (202, 1, 4, None),
            (365, 0, 3, None),
            (655, 2, 3, None),
            (795, 0, 4, None),
            (855, 2, 6, None),
            (1115, 0, 4, None),
            (1125, 2, 3, None),
            (1280, 0, 4, None),
        ],
    ),
    // Mayhem 16 "Fun Fair", after the published route with changes of our
    // own (saves 33 of 25).
    at(
        75,
        &[
            (100, 1, 7, None),
            (181, 2, 7, None),
            (222, 0, 3, None),
            (262, 3, 7, None),
            (343, 4, 7, None),
            (424, 5, 7, None),
            (430, 0, 3, None),
            (505, 6, 7, None),
            (627, 0, 1, Some(true)),
            (866, 1, 1, Some(false)),
            (1064, 7, 1, Some(true)),
            (1125, 3, 6, None),
            (1222, 2, 1, Some(false)),
            (1491, 4, 0, None),
            (1658, 6, 5, None),
            (1997, 15, 3, None),
            (2190, 15, 1, Some(false)),
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
            (265, 0, 1, Some(false)),
            (351, 1, 1, Some(true)),
            (495, 2, 1, Some(false)),
            (578, 3, 1, Some(true)),
            (693, 4, 1, Some(true)),
            (937, 5, 4, None),
            (1269, 9, 1, Some(false)),
            (1335, 10, 4, None),
            (1575, 10, 3, None),
        ],
    ),
    at(
        29,
        &[
            (38, 0, 6, None),
            (505, 3, 1, Some(true)),
            (749, 0, 1, Some(false)),
            (906, 4, 1, Some(true)),
            (1213, 5, 4, None),
            (1342, 5, 1, Some(false)),
            (1486, 6, 3, None),
            (1768, 6, 1, Some(true)),
            (1806, 7, 3, None),
            (1965, 7, 3, None),
        ],
    ),
    at(
        30,
        &[
            (89, 0, 1, Some(false)),
            (357, 1, 1, Some(false)),
            (625, 2, 1, Some(false)),
            (797, 3, 1, Some(false)),
            (1129, 4, 1, Some(true)),
            (1269, 5, 1, Some(true)),
            (1570, 6, 1, Some(true)),
            (1681, 7, 4, None),
            (1806, 7, 1, Some(true)),
            (2107, 8, 1, Some(true)),
            (2214, 9, 3, None),
            (2309, 9, 1, Some(true)),
            (2420, 10, 1, Some(true)),
            (2593, 11, 1, Some(true)),
            (2684, 12, 4, None),
            (2765, 12, 1, Some(true)),
            (2906, 13, 1, Some(true)),
            (3047, 14, 1, Some(true)),
            (3315, 15, 1, Some(true)),
            (3423, 16, 1, Some(true)),
            (3659, 17, 1, Some(false)),
            (3831, 18, 1, Some(false)),
        ],
    ),
    // Tricky 12 "Dot To Dot" by the published walkthrough.
    at(
        31,
        &[
            (113, 0, 1, Some(true)),
            (203, 1, 3, None),
            (362, 1, 3, None),
            (544, 1, 1, Some(false)),
            (599, 3, 3, None),
            (758, 3, 3, None),
            (939, 3, 1, Some(false)),
            (1024, 5, 3, None),
            (1185, 5, 3, None),
            (1364, 5, 1, Some(true)),
            (1451, 7, 3, None),
            (1613, 7, 3, None),
            (1839, 7, 1, Some(true)),
            (1878, 9, 3, None),
            (2044, 9, 3, None),
            (2229, 9, 1, Some(false)),
            (2306, 11, 3, None),
            (2472, 11, 3, None),
            (2654, 11, 1, Some(false)),
            (2739, 13, 1, Some(false)),
            (2857, 14, 1, Some(true)),
            (2975, 15, 1, Some(true)),
            (3092, 16, 1, Some(false)),
            (3176, 17, 3, None),
            (3342, 17, 3, None),
            (3536, 17, 3, None),
            (3649, 20, 3, None),
            (3705, 17, 3, None),
            (3888, 17, 1, Some(false)),
            (3973, 20, 1, Some(false)),
            (4017, 21, 1, Some(true)),
            (4116, 22, 3, None),
            (4282, 22, 3, None),
            (4515, 22, 3, None),
            (4624, 25, 3, None),
            (4680, 22, 3, None),
            (4856, 22, 1, Some(true)),
            (4948, 25, 1, Some(true)),
            (4995, 26, 1, Some(true)),
            (5144, 27, 1, Some(true)),
            (5263, 28, 1, Some(true)),
            (5358, 29, 3, None),
            (5524, 29, 3, None),
            (5704, 29, 1, Some(false)),
            (5754, 31, 3, None),
            (5920, 31, 3, None),
            (6101, 31, 1, Some(false)),
            (6150, 33, 3, None),
            (6316, 33, 3, None),
            (6544, 33, 3, None),
            (6710, 33, 3, None),
        ],
    ),
    at(
        33,
        &[
            (410, 0, 1, Some(true)),
            (480, 1, 4, None),
            (636, 1, 1, Some(true)),
            (900, 3, 1, Some(true)),
            (1079, 4, 1, Some(false)),
            (1265, 5, 1, Some(true)),
            (1661, 6, 3, None),
            (1852, 7, 1, Some(false)),
            (2364, 8, 5, None),
        ],
    ),
    // Taxing levels by the published walkthroughs; Taxing 11 "Chaos Maze" stops its digger
    // with a basher where the walkthrough gives a turner.
    at(
        43,
        &[
            (8, 2, 7, None),
            (9, 2, 8, None),
            (162, 2, 3, None),
            (572, 2, 3, None),
            (1062, 2, 3, None),
            (1194, 5, 4, None),
            (1560, 2, 3, None),
            (2048, 2, 3, None),
            (2242, 2, 4, None),
            (2316, 0, 4, None),
            (2317, 1, 4, None),
            (2362, 2, 1, Some(false)),
        ],
    ),
    at(
        45,
        &[
            (274, 0, 1, Some(false)),
            (1644, 1, 1, Some(true)),
            (1704, 3, 7, None),
            (1705, 3, 8, None),
            (1824, 5, 7, None),
            (1825, 5, 8, None),
            (2019, 3, 0, None),
            (2164, 5, 3, None),
            (2332, 3, 2, None),
            (2402, 7, 6, None),
        ],
    ),
    Solution {
        level: 46,
        plan: Plan::Rated(
            &[(250, 79)],
            &[
                (142, 0, 3, None),
                (412, 0, 1, Some(false)),
                (501, 4, 1, Some(true)),
                (875, 16, 1, Some(true)),
                (940, 20, 1, Some(true)),
                (1023, 24, 6, None),
                (1504, 2, 4, None),
                (1721, 3, 4, None),
                (2095, 1, 4, None),
                (3020, 2, 1, Some(true)),
            ],
        ),
        saves: 0,
    },
    at(
        50,
        &[
            (5, 0, 7, None),
            (197, 0, 6, None),
            (245, 0, 4, None),
            (286, 0, 1, Some(true)),
            (454, 3, 1, Some(true)),
            (659, 4, 1, Some(false)),
            (774, 5, 1, Some(true)),
            (1079, 6, 1, Some(true)),
            (1482, 7, 1, Some(true)),
            (1884, 8, 1, Some(true)),
            (2034, 9, 1, Some(false)),
            (2339, 10, 1, Some(true)),
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
            (89, 0, 3, None),
            (127, 1, 1, Some(true)),
            (261, 0, 3, None),
            (275, 2, 0, None),
            (406, 3, 1, Some(true)),
            (419, 0, 3, None),
            (495, 1, 2, None),
            (568, 0, 3, None),
            (657, 0, 6, None),
            (1007, 0, 4, None),
            (1120, 0, 6, None),
            (1344, 0, 1, Some(true)),
            (1377, 7, 4, None),
            (1487, 7, 1, Some(true)),
            (1581, 4, 4, None),
            (1633, 4, 6, None),
            (1815, 4, 4, None),
            (1865, 4, 1, Some(false)),
            (1944, 5, 4, None),
            (2064, 5, 1, Some(false)),
            (2191, 8, 4, None),
            (2371, 8, 1, Some(false)),
            (2461, 9, 4, None),
            (2646, 9, 1, Some(true)),
            (2731, 10, 4, None),
        ],
    ),
    at(
        47,
        &[
            (257, 1, 1, Some(true)),
            (642, 5, 1, Some(false)),
            (887, 9, 1, Some(false)),
            (1044, 53, 1, Some(false)),
            (1092, 13, 4, None),
            (1110, 57, 3, None),
            (1406, 61, 1, Some(false)),
            (1590, 0, 1, Some(false)),
            (1607, 8, 4, None),
            (1742, 57, 4, None),
            (1762, 2, 1, Some(true)),
            (1782, 33, 1, Some(true)),
            (1892, 28, 1, Some(true)),
            (2064, 36, 4, None),
            (2237, 44, 1, Some(false)),
        ],
    ),
    at(
        53,
        &[
            (94, 0, 1, Some(false)),
            (108, 1, 1, Some(false)),
            (333, 2, 1, Some(false)),
            (842, 4, 1, Some(false)),
            (941, 6, 1, Some(false)),
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
                (182, 0, 3, None),
                (546, 1, 4, None),
                (1212, 1, 4, None),
                (1512, 4, 4, None),
                (1677, 0, 2, None),
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
            (72, 1, 3, None),
            (420, 0, 5, None),
            (1141, 0, 3, None),
            (1232, 1, 2, None),
            (1302, 0, 5, None),
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
    // the trampolines were measured (re-timed for the counted hops).
    at(
        49,
        &[
            (188, 0, 4, None),
            (442, 0, 7, None),
            (518, 0, 3, None),
            (862, 0, 6, None),
            (1389, 5, 2, None),
            (1549, 6, 4, None),
            (1741, 6, 5, None),
            (2201, 6, 1, Some(true)),
            (2370, 37, 1, Some(true)),
            (2476, 38, 1, Some(false)),
            (2656, 13, 1, Some(false)),
            (2768, 17, 1, Some(true)),
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
            (464, 0, 3, None),
            (464, 2, 3, None),
            (476, 7, 3, None),
            (590, 1, 6, None),
            (623, 0, 3, None),
            (623, 2, 3, None),
            (788, 0, 3, None),
            (788, 2, 3, None),
            (817, 0, 4, None),
            (817, 2, 4, None),
            (866, 7, 3, None),
            (1100, 7, 2, None),
        ],
    ),
    // Mayhem 20 "Final Maze" by a route of our own.
    at(
        79,
        &[
            (116, 0, 1, Some(true)),
            (693, 1, 1, Some(false)),
            (803, 2, 4, None),
            (973, 4, 3, None),
            (1148, 4, 1, Some(true)),
            (1189, 6, 0, None),
            (1229, 7, 0, None),
            (2249, 7, 2, None),
            (2360, 8, 1, Some(false)),
            (2439, 9, 1, Some(true)),
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
