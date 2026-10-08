//! Searches single-skill solutions for a level: gives skill SKILL (id 0–8)
//! to lemming 0..LEMMINGS at every STEP-th tick from FROM to TO (turners on
//! both sides) and prints the assignments that save the most; an optional
//! first assignment `TICK LEMMING SKILL` is given before each one tried:
//! `cargo run --release -p l3d-sim --example search -- LEVEL SKILL LEMMINGS FROM TO STEP [TICK LEMMING SKILL]`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{Plan, Solution, play};

fn main() {
    let a: Vec<u64> = std::env::args().skip(1).map(|s| s.parse().expect("numbers")).collect();
    let (&[n, skill, lemmings, from, to, step], first) = a.split_at(6.min(a.len())) else { panic!("LEVEL SKILL LEMMINGS FROM TO STEP [TICK LEMMING SKILL]") };
    let first = match *first {
        [t, i, s] => Some((t, i as usize, s as u8, None)),
        [] => None,
        _ => panic!("the first assignment takes TICK LEMMING SKILL"),
    };
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n as u32).expect("level");
    let blocks = data.blocks(n as u32).expect("blocks");
    let sides: &[Option<bool>] = if skill == 1 { &[Some(true), Some(false)] } else { &[None] };
    let mut best = Vec::new();
    for t in (from..=to).step_by(step as usize) {
        for i in 0..lemmings as usize {
            for &side in sides {
                let steps: Vec<_> = first.into_iter().chain([(t, i, skill as u8, side)]).collect();
                let steps: &'static [(u64, usize, u8, Option<bool>)] = Box::leak(steps.into_boxed_slice());
                let sim = play(&Solution { level: n as u32, plan: Plan::At(steps), saves: 0 }, &level, &blocks);
                best.push((sim.counts.saved, t, i, side));
            }
        }
    }
    best.sort_by_key(|b| std::cmp::Reverse(b.0));
    for (saved, t, i, side) in best.iter().take(10) {
        println!("saved {saved:3}  tick {t} lemming {i} side {side:?}");
    }
}
