//! Runs every level for a simulated minute. Needs the user's game data (see
//! `docs/GROUNDRULES.md`); skipped when it is not present.

use std::path::{Path, PathBuf};

use l3d_formats::gamedata::{DATA_ENV, GameData};
use l3d_sim::{Simulation, TICKS_PER_SECOND};

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
