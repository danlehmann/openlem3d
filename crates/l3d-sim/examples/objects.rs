//! Lists a level's interactive objects (kinds 0x60-0x67) with the block in
//! their cell: `cargo run -p l3d-sim --example objects -- LEVEL...`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
fn main() {
    let mut data = GameData::open(&locate_data_dir(None)).expect("data");
    for n in std::env::args().skip(1).filter_map(|a| a.parse::<u32>().ok()) {
        let level = data.level(n).expect("level");
        println!("LEVEL.{n:03} {:?} trap type {}", level.title, level.trap_type);
        for (x, y, z, b, o) in level.cells() {
            if (0x60..=0x67).contains(&o.kind) {
                println!("  ({x:2},{y:2},{z:2}) kind {:#04x} extra {:#04x} block id {} shape {} rot {} segs {:04b}", o.kind, o.extra, b.id, b.shape, b.rotation, b.segments);
            }
        }
    }
}