//! Prints one lemming's position, heading and state every few ticks:
//! `cargo run -p l3d-sim --example trace -- LEVEL [LEMMING] [TICKS] [EVERY]`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::{SUB, Simulation};

fn main() {
    let mut args = std::env::args().skip(1).map(|a| a.parse::<u32>().unwrap_or(0));
    let n = args.next().unwrap_or(0);
    let which = args.next().unwrap_or(0) as usize;
    let ticks = args.next().filter(|&t| t > 0).unwrap_or(400);
    let every = args.next().filter(|&e| e > 0).unwrap_or(10);
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(n).expect("level");
    let blocks = data.blocks(n).expect("blocks");
    let mut sim = Simulation::new(&level, &blocks);
    let mut last = String::new();
    // Optional assignments from the environment, comma-separated:
    // ASSIGN=tick:lemming:skill[:cw|acw],...
    let spec = std::env::var("ASSIGN").unwrap_or_default();
    let assigns: Vec<Vec<&str>> = spec.split(',').filter(|a| !a.is_empty()).map(|a| a.split(':').collect()).collect();
    for t in 1..=ticks {
        sim.step();
        let now = sim.tick;
        for a in assigns.iter().filter(|a| a[0].parse() == Ok(now)) {
            let (i, s): (usize, u8) = (a[1].parse().expect("lemming"), a[2].parse().expect("skill"));
            let Some(skill) = l3d_sim::Skill::from_id(s) else { continue };
            let dir = sim.lemmings[i].dir;
            let ok = match a.get(3).copied() {
                Some("cw") => sim.assign_turner(i, dir.clockwise()),
                Some("acw") => sim.assign_turner(i, dir.anticlockwise()),
                _ => sim.assign(i, skill),
            };
            println!("assign {} to lemming {i}: {ok}", skill.name());
        }
        let Some(l) = sim.lemmings.get(which) else { continue };
        let p = l.pos.map(|v| v as f32 / SUB as f32);
        let line = format!("{:?} {:?}", l.dir, l.state);
        if t % every == 0 || line != last {
            println!("t={t:4} ({:6.2},{:5.2},{:6.2}) {line}", p[0], p[1], p[2]);
        }
        last = line;
        if l.gone {
            break;
        }
    }
    // Optional: the cells in a box at the end, DUMP=x0,x1,y0,y1,z0,z1.
    if let Ok(s) = std::env::var("DUMP") {
        let b: Vec<i32> = s.split(',').filter_map(|v| v.parse().ok()).collect();
        for y in (b[2]..=b[3]).rev() {
            for z in b[4]..=b[5] {
                let row: String = (b[0]..=b[1])
                    .map(|x| match sim.world.block([x, y, z]) {
                        Some((c, _)) if c.segments != 0 => format!("{:x}", c.segments),
                        _ => ".".into(),
                    })
                    .collect();
                println!("y={y} z={z} x{}..: {row}", b[0]);
            }
        }
    }
}
