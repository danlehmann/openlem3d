//! Plays every stored solution and prints, per level, how many lemmings
//! reached the exit, how many were needed, and the tick the level ended:
//! `cargo run --release -p l3d-sim --example solutions`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{SOLUTIONS, play};

fn main() {
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    for s in SOLUTIONS {
        let level = data.level(s.level).expect("level");
        let blocks = data.blocks(s.level).expect("blocks");
        let sim = play(s, &level, &blocks);
        let need = if s.saves > 0 { s.saves } else { level.save_requirement as u32 };
        println!("{:3} saved {:3} of {:3} need {:3} ended at tick {}", s.level, sim.counts.saved, level.lemmings, need, sim.tick);
    }
}
