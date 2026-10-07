//! Runs every level for a simulated minute. Needs the user's game data (see
//! `docs/GROUNDRULES.md`); skipped when it is not present.

use std::path::{Path, PathBuf};

use l3d_formats::gamedata::{DATA_ENV, GameData};
use l3d_formats::blk::BlockSet;
use l3d_formats::level::Level;
use l3d_sim::{Command, Simulation, TICKS_PER_SECOND, demos};

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

/// Every Practice level (and Fun 1) is solvable, and each solution's
/// command log replays to the same result (the demos play these logs).
#[test]
fn practice_solutions_still_work() {
    let Some(dir) = data_dir() else {
        eprintln!("game data not found; skipping");
        return;
    };
    let mut data = GameData::open(&dir).expect("open game data");
    let mut failures = Vec::new();
    for s in demos::SOLUTIONS {
        let level = data.level(s.level).expect("level");
        let blocks = data.blocks(s.level).expect("blocks");
        let played = demos::play(s, &level, &blocks);
        let need = if s.saves > 0 { s.saves } else { level.save_requirement as u32 };
        if played.counts.saved < need {
            failures.push(format!("LEVEL.{:03} saved {} of {need}", s.level, played.counts.saved));
        }
        let replayed = replay(&level, &blocks, &played.log, played.tick);
        if replayed.counts != played.counts {
            failures.push(format!("LEVEL.{:03}: replay differs", s.level));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// Runs a fresh simulation for `ticks`, applying `log`'s commands before
/// the step after the tick each was given on.
fn replay(level: &Level, blocks: &BlockSet, log: &[(u64, Command)], ticks: u64) -> Simulation {
    let mut sim = Simulation::new(level, blocks);
    let mut next = 0;
    while sim.tick < ticks {
        while let Some(&(t, c)) = log.get(next).filter(|(t, _)| *t <= sim.tick) {
            assert!(sim.apply(c), "command at tick {t} rejected on replay");
            next += 1;
        }
        sim.step();
    }
    sim
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
    let replayed = replay(&level, &blocks, &played.log, played.tick);
    assert_eq!(replayed.log, played.log);
    assert_eq!((replayed.counts, fingerprint(&replayed)), (played.counts, fingerprint(&played)));
}
