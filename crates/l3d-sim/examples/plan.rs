//! Plays a plan written as a stored solution would be and prints what it
//! saves: `cargo run --release -p l3d-sim --example plan -- LEVEL STEP...`.
//! Each step is `TICK:LEMMING:SKILL` (skill id 0–8, panel order; a turner
//! takes a fourth part, `cw` or `acw`) or `rTICK:CHANGE` for a release-rate
//! change, ticks counted from the hatches opening as in `demos.rs`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{parse_rate_step, parse_step, play_steps};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u32 = args.next().and_then(|a| a.parse().ok()).expect("LEVEL");
    let mut steps = Vec::new();
    let mut rates = Vec::new();
    for a in args {
        if a.starts_with('r') {
            rates.push(parse_rate_step(&a).expect("rTICK:CHANGE"));
        } else {
            steps.push(parse_step(&a).expect("TICK:LEMMING:SKILL"));
        }
    }
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n).expect("level");
    let blocks = data.blocks(n).expect("blocks");
    let sim = play_steps(&rates, &steps, &level, &blocks);
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
    // Optional: the lemmings still out at the end, with FATES set.
    if std::env::var_os("FATES").is_some() {
        for (i, l) in sim.lemmings.iter().enumerate().filter(|(_, l)| !l.gone) {
            let p = l.pos.map(|v| v as f32 / l3d_sim::SUB as f32);
            println!(
                "lemming {i}: {:?} at ({:.2}, {:.2}, {:.2}) heading {:?}",
                l.state, p[0], p[1], p[2], l.dir
            );
        }
    }
}
