//! Headless run of a level:
//! `cargo run -p l3d-sim --example run -- LEVEL [SECONDS] [TICK:LEMMING:SKILL ...]`.
//! Prints the counters once per simulated second and each lemming's fate.
//! Each `TICK:LEMMING:SKILL` assigns a skill (id 0–8, panel order) at a tick;
//! a turner takes a fourth part, `cw` or `acw`, for the side it points to.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{Step, apply_step, parse_step};
use l3d_sim::{SUB, Simulation, Skill, State, TICKS_PER_SECOND};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(0);
    let seconds: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(60);
    let assignments: Vec<(Step, Skill)> = args
        .filter_map(|a| {
            let step = parse_step(&a)?;
            Some((step, Skill::from_id(step.2)?))
        })
        .collect();
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n).expect("level");
    let blocks = data.blocks(n).expect("blocks");
    let mut sim = Simulation::new(&level, &blocks);
    println!(
        "LEVEL.{n:03} {:?}: {} entrances {:?}",
        level.title,
        sim.entrances.len(),
        sim.entrances
    );
    let skills: Vec<String> = Skill::ALL
        .iter()
        .filter(|s| sim.skills_left[**s as usize] > 0)
        .map(|s| format!("{} {}", s.name(), sim.skills_left[*s as usize]))
        .collect();
    println!(
        "skills: {}; lemmings {}, save {}",
        skills.join(", "),
        sim.to_release,
        level.save_requirement
    );
    for s in 1..=seconds {
        for _ in 0..TICKS_PER_SECOND {
            sim.step();
            for &(step @ (t, i, ..), skill) in &assignments {
                if t == sim.tick {
                    // Why a refusal happens: none left, or the lemming can't
                    // take it now (falling, busy, stunned, gone ...).
                    let why = match sim.lemmings.get(i) {
                        None => "no such lemming yet".to_string(),
                        Some(_) if sim.skills_left[skill as usize] == 0 => "none left".to_string(),
                        Some(l) => format!("lemming is {:?}", l.state),
                    };
                    if apply_step(&mut sim, step) {
                        println!("tick {t}: assign {} to lemming {i}: ok", skill.name());
                    } else {
                        println!(
                            "tick {t}: assign {} to lemming {i}: rejected ({why})",
                            skill.name()
                        );
                    }
                }
            }
        }
        let c = sim.counts;
        let states = |f: fn(&State) -> bool| {
            sim.lemmings
                .iter()
                .filter(|l| !l.gone && f(&l.state))
                .count()
        };
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
        println!(
            "lemming {i}: {:?} at ({:.2}, {:.2}, {:.2}) heading {:?} gone {}",
            l.state, p[0], p[1], p[2], l.dir, l.gone
        );
    }
}
