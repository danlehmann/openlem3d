//! Runs every level without skills and summarises what happened, to spot
//! collision bugs: `cargo run -p l3d-sim --example survey -- [SECONDS]`.
//! "stuck" counts walkers whose last 20 seconds stayed within half a unit
//! (lemmings trapped in a pocket the level never meant to have).

use std::collections::BTreeMap;

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::{SUB, Simulation, State, TICKS_PER_SECOND};

fn main() {
    let seconds: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(300);
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    for n in 0..100u32 {
        let (Ok(level), Ok(blocks)) = (data.level(n), data.blocks(n)) else { continue };
        let mut sim = Simulation::new(&level, &blocks);
        let window = (20 * TICKS_PER_SECOND) as usize;
        // Recent positions per lemming.
        let mut trails: Vec<Vec<[i32; 3]>> = Vec::new();
        let mut causes: BTreeMap<&'static str, u32> = BTreeMap::new();
        for _ in 0..seconds * TICKS_PER_SECOND {
            if sim.finished() {
                break;
            }
            sim.step();
            trails.resize(sim.lemmings.len(), Vec::new());
            for (i, l) in sim.lemmings.iter().enumerate() {
                let t = &mut trails[i];
                if !l.gone {
                    t.push(l.pos);
                    if t.len() > window {
                        t.remove(0);
                    }
                }
                if l.gone && !t.is_empty() {
                    let cause = match l.state {
                        State::Exiting => "saved",
                        State::Splatting => "splat",
                        State::Drowning => "drown",
                        State::Zapped => "zapped",
                        State::Exploding => "blown",
                        State::Trapped => "trapped",
                        _ => "other",
                    };
                    *causes.entry(cause).or_default() += 1;
                    t.clear();
                }
            }
        }
        let stuck: Vec<usize> = sim
            .lemmings
            .iter()
            .zip(&trails)
            .enumerate()
            .filter(|(_, (l, t))| !l.gone && t.len() == window && {
                let span = |i: usize| t.iter().map(|p| p[i]).max().unwrap() - t.iter().map(|p| p[i]).min().unwrap();
                span(0).max(span(1)).max(span(2)) < SUB / 2
            })
            .map(|(i, _)| i)
            .collect();
        let first = stuck.first().map(|&i| {
            let p = sim.lemmings[i].pos.map(|v| v as f32 / SUB as f32);
            format!(" (lemming {i} at {:.2},{:.2},{:.2})", p[0], p[1], p[2])
        });
        let states = sim.lemmings.iter().filter(|l| !l.gone).fold(BTreeMap::<String, u32>::new(), |mut m, l| {
            let s = format!("{:?}", l.state);
            *m.entry(s.split([' ', '{']).next().unwrap_or("").to_string()).or_default() += 1;
            m
        });
        println!(
            "{n:03} {:<30} need {:3}/{:3} | {:?} | out {:?} | stuck {}{}",
            level.title.trim(),
            level.save_requirement,
            level.lemmings,
            causes,
            states,
            stuck.len(),
            first.unwrap_or_default()
        );
    }
}
