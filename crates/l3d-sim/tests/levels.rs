//! Runs every level for a simulated minute. Needs the user's game data (see
//! `docs/GROUNDRULES.md`); skipped when it is not present.

use std::path::{Path, PathBuf};

use l3d_formats::gamedata::{DATA_ENV, GameData};
use l3d_sim::{Simulation, Skill, TICKS_PER_SECOND};

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os(DATA_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"));
    dir.is_dir().then_some(dir)
}

/// A compact fingerprint of a simulation's state.
fn fingerprint(sim: &Simulation) -> Vec<(i32, i32, i32, bool)> {
    sim.lemmings.iter().map(|l| (l.pos[0], l.pos[1], l.pos[2], l.gone)).collect()
}

#[test]
fn all_levels_run_deterministically() {
    let Some(dir) = data_dir() else {
        eprintln!("game data not found; skipping");
        return;
    };
    let mut data = GameData::open(&dir).expect("open game data");
    for n in 0..100 {
        let level = data.level(n).expect("level");
        let blocks = data.blocks(n).expect("blocks");
        let run = || {
            let mut sim = Simulation::new(&level, &blocks);
            for _ in 0..60 * TICKS_PER_SECOND {
                sim.step();
            }
            (sim.counts, fingerprint(&sim))
        };
        let (a, b) = (run(), run());
        assert_eq!(a, b, "LEVEL.{n:03} is not deterministic");
        assert!(a.0.released > 0 || level.lemmings == 0, "LEVEL.{n:03} released no lemmings");
    }
}

/// A Practice solution: `(tick, lemming, skill, turner side)` assignments,
/// or one skill for every lemming as it appears.
enum Plan {
    At(&'static [(u64, usize, u8, Option<bool>)]),
    Each(u8),
}

/// Solutions found in our simulation for each Practice level (and Fun 1),
/// with the number they save. Turner sides: `Some(true)` clockwise.
const SOLUTIONS: &[(u32, Plan, u32)] = &[
    (80, Plan::At(&[(140, 0, 0, None)]), 0),
    (81, Plan::At(&[(175, 0, 1, Some(true))]), 0),
    (82, Plan::At(&[(125, 0, 2, None)]), 0),
    (83, Plan::At(&[(185, 0, 3, None)]), 0),
    (84, Plan::At(&[(235, 0, 4, None)]), 0),
    (85, Plan::At(&[(150, 0, 5, None)]), 0),
    (86, Plan::At(&[(100, 0, 6, None)]), 0),
    (87, Plan::Each(7), 0),
    (88, Plan::Each(8), 0),
    (89, Plan::At(&[(66, 0, 4, None)]), 0),
    (90, Plan::At(&[(410, 0, 1, Some(true))]), 0),
    (91, Plan::At(&[(570, 0, 4, None)]), 0),
    (92, Plan::At(&[(100, 0, 3, None), (277, 0, 3, None)]), 0),
    (93, Plan::At(&[(122, 0, 4, None)]), 0),
    (94, Plan::At(&[(190, 0, 1, Some(false))]), 0),
    (95, Plan::At(&[(245, 0, 1, Some(true))]), 0),
    (96, Plan::At(&[(200, 0, 1, Some(false))]), 0),
    (97, Plan::At(&[(170, 0, 0, None)]), 0),
    (98, Plan::At(&[(343, 1, 1, Some(false))]), 18),
    (99, Plan::At(&[(1040, 0, 4, None)]), 0),
    (0, Plan::At(&[(1943, 0, 1, Some(false))]), 0),
];

/// Every Practice level (and Fun 1) is solvable: guards the mechanics the
/// solutions rely on. A save count of 0 means "at least the requirement".
#[test]
fn practice_solutions_still_work() {
    let Some(dir) = data_dir() else {
        eprintln!("game data not found; skipping");
        return;
    };
    let mut data = GameData::open(&dir).expect("open game data");
    let mut failures = Vec::new();
    for (n, plan, expect) in SOLUTIONS {
        let level = data.level(*n).expect("level");
        let blocks = data.blocks(*n).expect("blocks");
        let mut sim = Simulation::new(&level, &blocks);
        while !sim.finished() {
            sim.step();
            match plan {
                Plan::At(steps) => {
                    for &(t, i, s, side) in *steps {
                        if t != sim.tick {
                            continue;
                        }
                        let ok = match (side, sim.lemmings.get(i).map(|l| l.dir)) {
                            (Some(cw), Some(d)) => sim.assign_turner(i, if cw { d.clockwise() } else { d.anticlockwise() }),
                            _ => sim.assign(i, Skill::from_id(s).expect("skill")),
                        };
                        assert!(ok, "LEVEL.{n:03}: assignment at tick {t} rejected");
                    }
                }
                Plan::Each(s) => {
                    let skill = Skill::from_id(*s).expect("skill");
                    for i in 0..sim.lemmings.len() {
                        sim.assign(i, skill);
                    }
                }
            }
        }
        let need = if *expect > 0 { *expect } else { level.save_requirement as u32 };
        if sim.counts.saved < need {
            failures.push(format!("LEVEL.{n:03} saved {} of {need}", sim.counts.saved));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// Replaying a game's command log from the start reproduces it exactly.
#[test]
fn replaying_the_log_reproduces_the_game() {
    let Some(dir) = data_dir() else {
        eprintln!("game data not found; skipping");
        return;
    };
    let mut data = GameData::open(&dir).expect("open game data");
    let (level, blocks) = (data.level(98).expect("level"), data.blocks(98).expect("blocks"));
    let mut played = Simulation::new(&level, &blocks);
    for _ in 0..600 {
        played.step();
        match played.tick {
            40 => played.adjust_release_rate(30),
            343 => assert!(played.assign_turner(1, played.lemmings[1].dir.anticlockwise())),
            500 => played.nuke(),
            _ => {}
        }
    }
    assert_eq!(played.log.len(), 3);
    let mut replayed = Simulation::new(&level, &blocks);
    let mut next = 0;
    while replayed.tick < played.tick {
        replayed.step();
        while let Some(&(t, c)) = played.log.get(next).filter(|(t, _)| *t == replayed.tick) {
            assert!(replayed.apply(c), "command at tick {t} rejected on replay");
            next += 1;
        }
    }
    assert_eq!(replayed.log, played.log);
    assert_eq!((replayed.counts, fingerprint(&replayed)), (played.counts, fingerprint(&played)));
}
