//! Lists a level's sign and wall-decal objects, or with `all`, counts for
//! every level whether each side decal's own cell and the neighbour it names
//! are solid: `cargo run -p l3d-sim --example decals -- LEVEL|all`.

use l3d_formats::gamedata::{GameData, locate_data_dir};

/// Outward normals in decal side order: +X, −Z, −X, +Z.
const SIDES: [[i32; 2]; 4] = [[1, 0], [0, -1], [-1, 0], [0, 1]];

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    if arg != "all" {
        let level = data.level(arg.parse().unwrap_or(0)).expect("level");
        println!("flags {:#06x} walls set {}", level.flags, level.walls_set);
        for (x, y, z, b, o) in level.cells() {
            if (0x20..=0xEF).contains(&o.kind) {
                println!(
                    "({x:2},{y:2},{z:2}) object {:#04x} block id {} shape {} segs {:04b}",
                    o.kind, b.id, b.shape, b.segments
                );
            }
        }
        return;
    }
    // (own cell solid, neighbour solid) -> count
    let mut counts = std::collections::BTreeMap::new();
    for n in 0..100 {
        let Ok(level) = data.level(n) else { continue };
        let solid = |x: i32, y: i32, z: i32| {
            (0..32).contains(&x)
                && (0..32).contains(&z)
                && (0..16).contains(&y)
                && !level.block(x as usize, y as usize, z as usize).is_empty()
        };
        for (x, y, z, b, o) in level.cells() {
            if let 0x70..=0xAF = o.kind {
                let s = SIDES[((o.kind >> 4) - 7) as usize];
                let (x, y, z) = (x as i32, y as i32, z as i32);
                let next = solid(x + s[0], y, z + s[1]);
                *counts.entry((!b.is_empty(), next)).or_insert(0) += 1;
                if b.is_empty() {
                    println!(
                        "LEVEL.{n:03} ({x},{y},{z}) {:#04x}: own empty, neighbour {}",
                        o.kind,
                        if next { "solid" } else { "empty" }
                    );
                }
            }
        }
    }
    for ((own, next), c) in counts {
        println!("own solid {own:5} neighbour solid {next:5}: {c}");
    }
}
