//! Prints the raw block cells in a box: `cargo run -p l3d-sim --example cells -- LEVEL X0 X1 Y0 Y1 Z0 Z1`.

use l3d_formats::gamedata::{GameData, locate_data_dir};

fn main() {
    let a: Vec<usize> = std::env::args().skip(1).map(|a| a.parse().unwrap_or(0)).collect();
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let level = data.level(a[0] as u32).expect("level");
    for x in a[1]..=a[2] {
        for y in a[3]..=a[4] {
            for z in a[5]..=a[6] {
                let b = level.block(x, y, z);
                if !b.is_empty() {
                    println!("({x:2},{y:2},{z:2}) id {:2} shape {:2} rot {} segs {:04b}", b.id, b.shape, b.rotation, b.segments);
                }
            }
        }
    }
}
