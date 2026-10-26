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

    fn block(
        mut self,
        x: usize,
        y: usize,
        z: usize,
        id: u16,
        rotation: u16,
        segments: u16,
    ) -> Self {
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
    Synth::new()
        .floor(1, 2..30, 2..30)
        .hatch(16, 3, 16, rotation)
}

#[test]
fn release_interval_is_101_minus_rate() {
    let mut sim = open_level(1).rate(80).sim();
    // Nothing drops while the hatch opens; the first on the tick after.
    run(&mut sim, l3d_sim::HATCH_OPEN_TICKS);
    assert_eq!(sim.counts.released, 0);
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
    for (rotation, dir) in [
        (0, Dir::NegZ),
        (1, Dir::PosX),
        (2, Dir::PosZ),
        (3, Dir::NegX),
    ] {
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
    assert_eq!(
        sim.lemmings[0].pos[0] - x0,
        32 * SUB / 32,
        "one unit in 32 ticks"
    );
}

#[test]
fn two_unit_drop_survives_and_five_unit_drop_splats() {
    for (drop, survives) in [(2, true), (5, false)] {
        // Walk +X off a ledge at layer 7 onto a floor `drop` units lower.
        let top = 7;
        let low = top - drop;
        let mut s = Synth::new()
            .floor(top, 10..14, 15..18)
            .hatch(11, top + 2, 16, 1);
        s = s.floor(low, 14..30, 2..30);
        let mut sim = s.sim();
        run(&mut sim, 250);
        let l = &sim.lemmings[0];
        if survives {
            assert!(
                !l.gone && !matches!(l.state, State::Splatting),
                "drop {drop}: {:?}",
                l.state
            );
        } else {
            assert!(
                l.gone || matches!(l.state, State::Splatting),
                "drop {drop}: {:?}",
                l.state
            );
        }
    }
}

#[test]
fn builder_lays_six_bricks() {
    let mut sim = open_level(1).skill(0, Skill::Builder, 1).sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Builder));
    // It walks on to the middle of its cell or its edge first.
    let cell = sim.lemmings[0].pos.map(|v| v.div_euclid(SUB));
    while !matches!(sim.lemmings[0].state, State::Building { .. }) {
        assert!(sim.tick < 30 + SUB as u64, "never started building");
        sim.step();
    }
    let start = sim.lemmings[0].pos;
    let at = start.map(|v| v.div_euclid(SUB));
    assert!(at[0].abs_diff(cell[0]) + at[2].abs_diff(cell[2]) <= 1);
    // At the middle of its cell or its edge, whichever came first.
    let on = |v: i32| [0, SUB / 2, SUB - 1].contains(&v.rem_euclid(SUB));
    assert!(on(start[0]) && on(start[2]), "started at {start:?}");
    while sim.lemmings[0].state != State::Shrugging {
        assert!(sim.tick < 30 + SUB as u64 + 6 * 25 + 20, "never finished");
        sim.step();
    }
    assert_eq!(sim.world.bricks.len(), 6);
    // Each brick: ¼ unit up and ½ unit forward; the builder stands at the
    // start of its last brick.
    let end = sim.lemmings[0].pos;
    assert_eq!(end[1] - start[1], 6 * SUB / 4);
    let run_len = (end[0] - start[0]).abs() + (end[2] - start[2]).abs();
    assert_eq!(run_len, 5 * SUB / 2);
}

#[test]
fn walkers_cross_a_staircase_from_the_side() {
    // Walkers head +X on the floor (surface y = 2) and meet, side-on, a low
    // step (top ¼ up) and then the next one (from ¼ to ½ up): they step over
    // the first and pass under the second, never turning round.
    let mut sim = open_level(1).sim();
    let floor = 2 * SUB;
    let brick = |x: i32, bottom: i32| l3d_sim::Brick {
        min: [x * SUB, floor + bottom, 14 * SUB],
        max: [x * SUB + SUB / 2, floor + bottom + SUB / 4, 19 * SUB],
        id: 9,
    };
    assert!(sim.world.add_brick(brick(20, 0)));
    assert!(sim.world.add_brick(brick(22, SUB / 4)));
    run(&mut sim, 300);
    let l = &sim.lemmings[0];
    assert_eq!(l.dir, Dir::PosX);
    assert!(l.pos[0] > 23 * SUB, "x = {}", l.pos[0] as f32 / SUB as f32);
}

#[test]
fn bashing_removes_bricks() {
    let mut sim = open_level(1).sim();
    let c = [20, 2, 20];
    let b = l3d_sim::Brick {
        min: [c[0] * SUB + 10, c[1] * SUB, c[2] * SUB],
        max: [c[0] * SUB + 140, c[1] * SUB + SUB / 4, c[2] * SUB + SUB],
        id: 9,
    };
    assert!(sim.world.add_brick(b));
    assert!(
        sim.world
            .solid([c[0] * SUB + 20, c[1] * SUB + 10, c[2] * SUB + 128])
    );
    assert!(sim.world.remove_segments(c, 0b0001));
    assert!(sim.world.bricks.is_empty());
}

#[test]
fn one_way_blocks_give_way_in_one_direction() {
    // Block 6 is one-way towards −X before rotation; rotation 1 turns −X to +Z.
    for (rotation, open, closed) in [(0, [-1, 0, 0], [1, 0, 0]), (1, [0, 0, 1], [0, 0, -1])] {
        let mut sim = open_level(1).block(20, 2, 20, 6, rotation, 0xF).sim();
        let c = [20, 2, 20];
        assert!(
            !sim.world.remove_segments_towards(c, 0b0011, Some(closed)),
            "rotation {rotation}: closed side"
        );
        assert!(
            !sim.world.remove_segments_towards(c, 0b0011, None),
            "rotation {rotation}: digging down"
        );
        assert!(
            sim.world.remove_segments_towards(c, 0b0011, Some(open)),
            "rotation {rotation}: open side"
        );
    }
}

/// Runs until lemming `i`'s builder has `left` bricks still to lay.
fn build_until(sim: &mut Simulation, i: usize, left: u8) {
    while sim.lemmings[i].state != (State::Building { bricks_left: left }) {
        assert!(sim.tick < 1000, "builder never got to {left} bricks left");
        sim.step();
    }
}

#[test]
fn builder_takes_a_turner_only_where_its_step_ends_at_a_centre() {
    let mut sim = open_level(1)
        .skill(0, Skill::Builder, 1)
        .skill(1, Skill::Turner, 2)
        .sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Builder));
    while !matches!(sim.lemmings[0].state, State::Building { .. }) {
        sim.step();
    }
    let start = sim.lemmings[0].pos;
    // Whether the end of brick `n` lies at a cell's centre.
    let centre_after = |n: i32| {
        let off = (start[0] + n * SUB / 2).rem_euclid(SUB) - SUB / 2;
        off.abs() < SUB / 4
    };
    let (odd, even) = if centre_after(4) { (3, 4) } else { (4, 5) };
    let to = Dir::PosX.anticlockwise();
    // Just laid, the odd brick ends at an edge: refused, the turner kept.
    build_until(&mut sim, 0, 6 - odd);
    assert!(!sim.assign_turner(0, to));
    assert_eq!(sim.skills_left[Skill::Turner as usize], 2);
    // Just laid, before the builder climbs it, the even brick ends at a
    // centre: taken, and the builder stands at its end.
    build_until(&mut sim, 0, 6 - even);
    assert!(sim.assign_turner(0, to));
    assert_eq!(sim.skills_left[Skill::Turner as usize], 1);
    let l = &sim.lemmings[0];
    assert!(matches!(l.state, State::Turning { .. }), "{:?}", l.state);
    assert_eq!(l.pos[0], start[0] + even as i32 * SUB / 2);
    assert_eq!(l.pos[1], start[1] + even as i32 * SUB / 4);
    run(&mut sim, 30);
    let l = &sim.lemmings[0];
    assert!(matches!(l.state, State::Turning { .. }), "{:?}", l.state);
}

#[test]
fn builder_reaching_a_turner_half_a_unit_up_is_turned() {
    let mut sim = open_level(1)
        .skill(0, Skill::Builder, 1)
        .skill(1, Skill::Turner, 1)
        .sim();
    let to = Dir::PosX.anticlockwise();
    // The first lemming made a turner in cell x = 19.
    while sim.lemmings.is_empty()
        || sim.lemmings[0].state != State::Walking
        || sim.lemmings[0].pos[0].div_euclid(SUB) != 19
    {
        sim.step();
    }
    assert!(sim.assign_turner(0, to));
    // The next one builds from the centre of cell 18.
    while sim.lemmings.len() < 2
        || sim.lemmings[1].state != State::Walking
        || sim.lemmings[1].pos[0].div_euclid(SUB) != 18
    {
        assert!(sim.tick < 1000, "second lemming never reached cell 18");
        sim.step();
    }
    assert!(sim.assign(1, Skill::Builder));
    while matches!(
        sim.lemmings[1].state,
        State::Walking | State::Building { .. }
    ) && sim.lemmings[1].dir == Dir::PosX
    {
        assert!(sim.tick < 1000, "builder never turned");
        sim.step();
    }
    let l = &sim.lemmings[1];
    assert_eq!(l.state, State::Walking);
    assert_eq!(l.dir, to);
    assert_eq!(sim.world.bricks.len(), 2);
    assert_eq!(l.pos[0], 19 * SUB + SUB / 2);
}

#[test]
fn builder_becomes_a_blocker_where_its_step_ends_at_a_centre() {
    let mut sim = open_level(1)
        .skill(0, Skill::Builder, 1)
        .skill(1, Skill::Blocker, 2)
        .sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Builder));
    while !matches!(sim.lemmings[0].state, State::Building { .. }) {
        assert!(sim.tick < 500, "no builder: {:?}", sim.lemmings[0].state);
        sim.step();
    }
    let start = sim.lemmings[0].pos;
    let centre_after = |n: i32| {
        let off = (start[0] + n * SUB / 2).rem_euclid(SUB) - SUB / 2;
        off.abs() < SUB / 4
    };
    let (odd, even) = if centre_after(4) { (3, 4) } else { (4, 5) };
    build_until(&mut sim, 0, 6 - odd);
    assert!(!sim.assign(0, Skill::Blocker));
    build_until(&mut sim, 0, 6 - even);
    assert!(sim.assign(0, Skill::Blocker));
    let l = &sim.lemmings[0];
    assert_eq!(l.state, State::Blocking);
    assert_eq!(l.pos[0], start[0] + even as i32 * SUB / 2);
    // It stands there, on the far edge of its newest brick.
    run(&mut sim, 30);
    assert_eq!(sim.lemmings[0].state, State::Blocking);
}

#[test]
fn walker_turns_back_at_the_start_of_a_staircase_blockers_step() {
    let mut sim = open_level(1)
        .rate(1)
        .skill(0, Skill::Builder, 1)
        .skill(1, Skill::Blocker, 1)
        .sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Builder));
    while !matches!(sim.lemmings[0].state, State::Building { .. }) {
        assert!(sim.tick < 500, "no builder: {:?}", sim.lemmings[0].state);
        sim.step();
    }
    let start = sim.lemmings[0].pos;
    let centre_after = |n: i32| {
        let off = (start[0] + n * SUB / 2).rem_euclid(SUB) - SUB / 2;
        off.abs() < SUB / 4
    };
    let even = if centre_after(4) { 4 } else { 5 };
    build_until(&mut sim, 0, 6 - even);
    assert!(sim.assign(0, Skill::Blocker));
    let blocker = sim.lemmings[0].pos;
    // The next lemming climbs the staircase and is turned back as it
    // reaches the blocker's step (the owner on Fun 13, 2026-10-24).
    while sim.lemmings.len() < 2 || sim.lemmings[1].dir == Dir::PosX {
        assert!(!sim.finished() && sim.tick < 2000, "never turned back");
        sim.step();
    }
    let l = &sim.lemmings[1];
    assert_eq!(l.dir, Dir::NegX);
    let gap = blocker[0] - l.pos[0];
    assert!(
        (SUB / 2 - 8..=SUB / 2).contains(&gap),
        "turned {gap} before the blocker"
    );
}

#[test]
fn miner_on_its_way_takes_a_turner_and_a_blocker() {
    for skill in [Skill::Turner, Skill::Blocker] {
        let mut sim = open_level(1)
            .skill(0, Skill::Miner, 1)
            .skill(1, skill, 1)
            .sim();
        run(&mut sim, 30);
        assert!(sim.assign(0, Skill::Miner));
        let taken = if skill == Skill::Turner {
            sim.assign_turner(0, Dir::PosX.anticlockwise())
        } else {
            sim.assign(0, skill)
        };
        assert!(taken, "{skill:?} refused");
        run(&mut sim, 40);
        assert!(
            matches!(
                sim.lemmings[0].state,
                State::Turning { .. } | State::Blocking
            ),
            "{:?}",
            sim.lemmings[0].state
        );
    }
}

/// The furthest x lemming 0 reaches walking +X from the hatch towards a cell
/// at (20, 2, 16) with the given segments.
fn furthest_x(segments: u16) -> i32 {
    let mut sim = open_level(1).block(20, 2, 16, FLOOR, 0, segments).sim();
    let mut x = 0;
    for _ in 0..400 {
        sim.step();
        if let Some(l) = sim.lemmings.first() {
            x = x.max(l.pos[0]);
        }
    }
    x
}

#[test]
fn walkers_climb_a_quarter_step_and_turn_at_half() {
    assert!(furthest_x(0b0001) > 21 * SUB, "a ¼ step is climbed");
    assert!(furthest_x(0b0011) < 20 * SUB, "a ½ step turns walkers");
}

#[test]
fn walkers_need_half_a_unit_of_headroom_and_walk_in_under_less() {
    assert!(
        furthest_x(0b1100) > 21 * SUB,
        "½ unit of headroom is enough"
    );
    // With ¼ they walk on until their centre reaches the cell's edge.
    let x = furthest_x(0b1110);
    assert!(
        (20 * SUB - SUB / 16..=20 * SUB).contains(&x),
        "turned at {x}"
    );
}

/// Lemming 0 as a climber walking +X from the hatch into a two-unit wall at
/// x = 20 (layers 2 and 3) with `top` set at layer 4; how far it gets, how
/// high, and how many are saved after 600 ticks.
fn climb_wall(top: Option<(u16, u16, u16)>) -> (i32, i32, u32) {
    let mut synth = open_level(1)
        .block(20, 2, 16, FLOOR, 0, 0xF)
        .block(20, 3, 16, FLOOR, 0, 0xF);
    if let Some((id, rotation, segments)) = top {
        synth = synth.block(20, 4, 16, id, rotation, segments);
    }
    let mut sim = synth.skill(0, Skill::Climber, 1).sim();
    run(&mut sim, 30);
    assert!(sim.assign(0, Skill::Climber));
    let (mut x, mut y) = (0, 0);
    for _ in 0..600 {
        sim.step();
        let l = &sim.lemmings[0];
        x = x.max(l.pos[0]);
        y = y.max(l.pos[1]);
    }
    (x, y, sim.counts.saved)
}

#[test]
fn climber_lets_go_with_no_room_at_the_top() {
    let (x, _, _) = climb_wall(None);
    assert!(x > 21 * SUB, "over an open top");
    // A block ¼ above the top leaves no room for the body.
    let (x, y, _) = climb_wall(Some((FLOOR, 0, 0b1110)));
    assert!(x < 20 * SUB, "stayed on its side: x = {x}");
    assert!(y < 4 * SUB + SUB / 4, "no higher than the gap: y = {y}");
}

#[test]
fn climber_goes_into_an_exit_doorway_at_the_top() {
    // Rotation 3: the doorway faces −X, towards the climber.
    let (_, y, saved) = climb_wall(Some((1, 3, 0xF)));
    assert_eq!(saved, 1);
    assert!(y < 4 * SUB + SUB / 4, "went in at the top: y = {y}");
}
