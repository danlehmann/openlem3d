//! Headless run of a level:
//! `cargo run -p l3d-sim --example run -- LEVEL [SECONDS] [TICK:LEMMING:SKILL ...]`.
//! Prints the counters once per simulated second and each lemming's fate.
//! Each `TICK:LEMMING:SKILL` assigns a skill (id 0–8, panel order) at a tick.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::{SUB, Simulation, Skill, State, TICKS_PER_SECOND};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(0);
    let seconds: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(60);
    let assignments: Vec<(u64, usize, Skill)> = args
        .filter_map(|a| {
            let mut p = a.split(':').map(|v| v.parse::<u64>().ok());
            Some((p.next()??, p.next()?? as usize, Skill::from_id(p.next()?? as u8)?))
        })
        .collect();
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n).expect("level");
    let blocks = data.blocks(n).expect("blocks");
    let mut sim = Simulation::new(&level, &blocks);
    println!("LEVEL.{n:03} {:?}: {} entrances {:?}", level.title, sim.entrances.len(), sim.entrances);
    for s in 1..=seconds {
        for _ in 0..TICKS_PER_SECOND {
            sim.step();
            for &(t, i, skill) in &assignments {
                if t == sim.tick {
                    let ok = sim.assign(i, skill);
                    println!("tick {t}: assign {} to lemming {i}: {}", skill.name(), if ok { "ok" } else { "rejected" });
                }
            }
        }
        let c = sim.counts;
        let states = |f: fn(&State) -> bool| sim.lemmings.iter().filter(|l| !l.gone && f(&l.state)).count();
        println!(
            "t={s:3}s released {:2} out {:2} saved {:2} dead {:2} | walking {:2} falling {:2}",
            c.released,
            sim.out(),
            c.saved,
            c.dead,
            states(|s| matches!(s, State::Walking)),
            states(|s| matches!(s, State::Falling { .. })),
        );
    }
    let changes = sim.world.take_changes();
    println!("terrain cells changed: {}", changes.len());
    for (c, b) in changes.iter().take(12) {
        println!("  cell {c:?} -> segments {:04b}", b.segments);
    }
    for (i, l) in sim.lemmings.iter().enumerate().take(10) {
        let p = l.pos.map(|v| v as f32 / SUB as f32);
        println!("lemming {i}: {:?} at ({:.2}, {:.2}, {:.2}) heading {:?} gone {}", l.state, p[0], p[1], p[2], l.dir, l.gone);
    }
}
