//! Plays a plan written as a stored solution would be and prints what it
//! saves: `cargo run --release -p l3d-sim --example plan -- LEVEL STEP...`.
//! Each step is `TICK:LEMMING:SKILL` (skill id 0–8, panel order; a turner
//! takes a fourth part, `cw` or `acw`) or `rTICK:CHANGE` for a release-rate
//! change, ticks counted from the hatches opening as in `demos.rs`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{Plan, Solution, play};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u32 = args.next().and_then(|a| a.parse().ok()).expect("LEVEL");
    let mut steps = Vec::new();
    let mut rates = Vec::new();
    for a in args {
        if let Some(r) = a.strip_prefix('r') {
            let (t, c) = r.split_once(':').expect("rTICK:CHANGE");
            rates.push((t.parse().unwrap(), c.parse().unwrap()));
            continue;
        }
        let p: Vec<&str> = a.split(':').collect();
        let side = match p.get(3) {
            Some(&"cw") => Some(true),
            Some(&"acw") => Some(false),
            _ => None,
        };
        steps.push((
            p[0].parse().unwrap(),
            p[1].parse().unwrap(),
            p[2].parse().unwrap(),
            side,
        ));
    }
    let steps: &'static [_] = steps.leak();
    let rates: &'static [_] = rates.leak();
    let plan = if rates.is_empty() {
        Plan::At(steps)
    } else {
        Plan::Rated(rates, steps)
    };
    let s = Solution {
        level: n,
        plan,
        saves: 0,
    };
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n).expect("level");
    let blocks = data.blocks(n).expect("blocks");
    let sim = play(&s, &level, &blocks);
    let c = sim.counts;
    println!(
        "LEVEL.{n:03} {:?}: saved {} of {} needed ({} lemmings), dead {}, ended at tick {}; {} of {} assignments taken",
        level.title,
        c.saved,
        level.save_requirement,
        level.lemmings,
        c.dead,
        sim.tick,
        sim.log.len(),
        steps.len() + rates.len(),
    );
}
