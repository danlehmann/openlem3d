//! Behaviour measured in the original game (`docs/spec/behaviour.md`),
//! checked on small synthetic levels so no game data is needed.

use l3d_formats::blk::{BLOCKS, BlockSet, DEF_LEN};
use l3d_formats::level::{FILE_LEN, HEADER_LEN, Level, cell_index};
use l3d_sim::{Dir, SUB, Simulation, Skill, State};

/// Block id of the ordinary floor/wall block in the synthetic levels.
const FLOOR: u16 = 9;

/// A synthetic level builder: an empty 32×16×32 grid with a solid level
/// bottom, plus the cells set by the test.
struct Synth {
    bytes: Vec<u8>,
}

impl Synth {
    fn new() -> Self {
        let mut bytes = vec![0u8; FILE_LEN];
        bytes[1] = 10; // 10 minutes
        bytes[0xE2..0xE4].copy_from_slice(&50u16.to_le_bytes()); // release rate
        bytes[0xE4..0xE6].copy_from_slice(&10u16.to_le_bytes()); // lemmings
        bytes[0x11B..0x11D].copy_from_slice(&0x0080u16.to_le_bytes()); // bottom solid
        bytes[0x153..0x157].copy_from_slice(&[0, 0, 31, 31]); // kill boundary
        bytes[0x15C] = 15; // kill ceiling
        Synth { bytes }
    }

    fn rate(mut self, r: u16) -> Self {
        self.bytes[0xE2..0xE4].copy_from_slice(&r.to_le_bytes());
        self
    }

    fn skill(mut self, slot: usize, skill: Skill, n: u8) -> Self {
        self.bytes[0x0A + 2 * slot] = skill as u8;
        self.bytes[0x0B + 2 * slot] = n;
        self
    }

    fn block(mut self, x: usize, y: usize, z: usize, id: u16, rotation: u16, segments: u16) -> Self {
        let raw = id << 10 | rotation << 4 | segments;
        let o = HEADER_LEN + cell_index(x, y, z) * 2;
        self.bytes[o..o + 2].copy_from_slice(&raw.to_le_bytes());
        self
    }

    /// A floor slab of full blocks at layer `y` over x in `xs`, z in `zs`.
    fn floor(mut self, y: usize, xs: std::ops::Range<usize>, zs: std::ops::Range<usize>) -> Self {
        for x in xs {
            for z in zs.clone() {
                self = self.block(x, y, z, FLOOR, 0, 0xF);
            }
        }
        self
    }

    /// An entrance hatch (top two segments) at `(x, y, z)` with `rotation`.
    fn hatch(self, x: usize, y: usize, z: usize, rotation: u16) -> Self {
        self.block(x, y, z, 0, rotation, 0b1100)
    }

    fn sim(&self) -> Simulation {
        let level = Level::parse(&self.bytes).expect("synthetic level");
        let mut blk = vec![0u8; BLOCKS * DEF_LEN];
        // Block 9: an ordinary block with real textures (not a placeholder).
        blk[9 * DEF_LEN + 4..10 * DEF_LEN].fill(1);
        Simulation::new(&level, &BlockSet::parse(&blk).expect("synthetic blk"))
    }
}

fn run(sim: &mut Simulation, ticks: u32) {
    for _ in 0..ticks {
        sim.step();
    }
}

/// A big open floor at layer 1 (walking surface y = 2) with a hatch above.
fn open_level(rotation: u16) -> Synth {
    Synth::new().floor(1, 2..30, 2..30).hatch(16, 3, 16, rotation)
}

#[test]
fn release_interval_is_101_minus_rate() {
    let mut sim = open_level(1).rate(80).sim();
    sim.step();
    assert_eq!(sim.counts.released, 1);
    // The next release comes 101 − 80 = 21 ticks later.
    run(&mut sim, 20);
    assert_eq!(sim.counts.released, 1);
    run(&mut sim, 1);
    assert_eq!(sim.counts.released, 2);
}

#[test]
fn release_direction_by_rotation() {
    for (rotation, dir) in [(0, Dir::NegZ), (1, Dir::PosX), (2, Dir::PosZ), (3, Dir::NegX)] {
        let sim = open_level(rotation).sim();
        assert_eq!(sim.entrances[0].dir, dir, "rotation {rotation}");
    }
}

#[test]
fn walking_speed_is_one_32nd_unit_per_tick() {
    let mut sim = open_level(1).sim();
    // Let the first lemming land and start walking.
    run(&mut sim, 30);
    let l = &sim.lemmings[0];
    assert_eq!(l.state, State::Walking);
    let x0 = l.pos[0];
    run(&mut sim, 32);
    assert_eq!(sim.lemmings[0].pos[0] - x0, 32 * SUB / 32, "one unit in 32 ticks");
}

#[test]
fn two_unit_drop_survives_and_five_unit_drop_splats() {
    for (drop, survives) in [(2, true), (5, false)] {
        // Walk +X off a ledge at layer 7 onto a floor `drop` units lower.
        let top = 7;
        let low = top - drop;
        let mut s = Synth::new().floor(top, 10..14, 15..18).hatch(11, top + 2, 16, 1);
        s = s.floor(low, 14..30, 2..30);
        let mut sim = s.sim();
        run(&mut sim, 250);
        let l = &sim.lemmings[0];
        if survives {
            assert!(!l.gone && !matches!(l.state, State::Splatting), "drop {drop}: {:?}", l.state);
        } else {
            assert!(l.gone || matches!(l.state, State::Splatting), "drop {drop}: {:?}", l.state);
        }
    }
}

#[test]
fn builder_lays_six_bricks() {
    let mut sim = open_level(1).skill(0, Skill::Builder, 1).sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Builder));
    sim.world.take_changes();
    run(&mut sim, 6 * 25 + 10);
    let added = sim.world.take_changes().len();
    assert_eq!(added, 6, "one terrain change per brick");
}
