//! Makes a changed copy of the game CD for experiments in the original game
//! (DOSBox-X loads it through `LEM3D_CUE`), never touching the user's image:
//!
//! - `cdcopy make DIR`: copies the data track to `DIR/data.iso` (2048-byte
//!   sectors) and writes `DIR/patched.cue`, whose audio tracks still come
//!   from the original image.
//! - `cdcopy level DIR LEVEL [X,Y,Z=ID:SHAPE:ROT:SEGS ...]`: writes
//!   `LEVELS/LEVEL.nnn`, unpacked, with the given block-grid cells changed
//!   (`docs/spec/level.md`), to the end of the copy and points the file's
//!   directory record at it. Each run starts again from the original level;
//!   `LEVEL=OTHER` starts from level OTHER's file instead, so `98=68` puts
//!   Mayhem 9 in the Practice "Trampoline" slot. The slot also gets the
//!   block file `BLK.nnn` of the level written, which defines its blocks.
//!   An edit `@OFFSET=B,B,...` writes raw bytes (hex) at a hex offset of the
//!   unpacked level, e.g. a preset camera (`docs/spec/level.md`).
//!   An edit `#ID=TILE:SHADE[:FLAGS]` redefines block ID in the slot's block
//!   file: that tile and shading on all six faces.
//!
//! The copy holds game data: keep it under the git-ignored `temp/`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use l3d_formats::disc::{Disc, SECTOR_SIZE};
use l3d_formats::gamedata::{GameData, locate_data_dir};

const GRID: usize = 0x200;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(args.get(1).expect("DIR"));
    match args[0].as_str() {
        "make" => make(dir),
        "level" => {
            let (n, from) = args[2].split_once('=').unwrap_or((&args[2], &args[2]));
            let n: u32 = n.parse().expect("level number");
            let from: u32 = from.parse().expect("level number");
            level(dir, n, from, &args[3..]);
        }
        "mark" => mark(dir, &args[2..], true),
        "unmark" => mark(dir, &args[2..], false),
        a => panic!("unknown command {a}"),
    }
}

fn level(dir: &Path, n: u32, from: u32, edits: &[String]) {
    let mut data = GameData::open(&locate_data_dir(None)).expect("game CD");
    let mut level = data.read(&format!("LEVELS/LEVEL.{from:03}")).unwrap();
    for e in edits {
        if e.starts_with('#') {
            continue;
        }
        if let Some(raw) = e.strip_prefix('@') {
            // `@OFFSET=B,B,...`: bytes (hex) written at a hex offset.
            let (at, bytes) = raw.split_once('=').expect("@OFFSET=B,B,...");
            let at = usize::from_str_radix(at.trim_start_matches("0x"), 16).unwrap();
            for (k, b) in bytes.split(',').enumerate() {
                level[at + k] = u8::from_str_radix(b, 16).unwrap();
            }
            println!("bytes at {at:#x}: {bytes}");
            continue;
        }
        let (at, cell) = e.split_once('=').expect("X,Y,Z=ID:SHAPE:ROT:SEGS");
        let p: Vec<usize> = at.split(',').map(|v| v.parse().unwrap()).collect();
        let c: Vec<u16> = cell.split(':').map(|v| v.parse().unwrap()).collect();
        let raw = (c[0] << 10) | ((c[1] & 15) << 6) | ((c[2] & 3) << 4) | (c[3] & 15);
        let i = GRID + ((p[0] * 16 + p[1]) * 32 + p[2]) * 2;
        println!(
            "cell {at}: {:04x} -> {raw:04x}",
            u16::from_le_bytes([level[i], level[i + 1]])
        );
        level[i..i + 2].copy_from_slice(&raw.to_le_bytes());
    }
    put(dir, &mut data, &format!("LEVEL.{n:03}"), &level);
    // The level's own blocks go with it: a slot's BLK may lack blocks the
    // level uses (one-way blocks, splitters). Rerunning `N` puts its own
    // back.
    let mut blk = data.read(&format!("LEVELS/BLK.{from:03}")).unwrap();
    // `#ID=TILE:SHADE[:FLAGS]`: block ID with that tile and shading on all
    // six faces (`docs/spec/blk.md`).
    for e in edits.iter().filter_map(|e| e.strip_prefix('#')) {
        let (id, def) = e.split_once('=').expect("#ID=TILE:SHADE[:FLAGS]");
        let id: usize = id.parse().unwrap();
        let v: Vec<u8> = def.split(':').map(|v| v.parse().unwrap()).collect();
        let d = &mut blk[id * 22..id * 22 + 22];
        d[..4].copy_from_slice(&[0, 0, v.get(2).copied().unwrap_or(0), 0]);
        for face in d[4..].chunks_mut(3) {
            face.copy_from_slice(&[v[0], 0, v[1]]);
        }
        println!("block {id}: tile {} shading {}", v[0], v[1]);
    }
    put(dir, &mut data, &format!("BLK.{n:03}"), &blk);
}

/// Writes `bytes` as file `LEVELS/name`, unpacked, to the end of the copy and
/// points its directory record there.
fn put(dir: &Path, data: &mut GameData, name: &str, bytes: &[u8]) {
    put_in(dir, data, "LEVELS", name, bytes);
}

/// Writes `bytes` as file `folder/name`, unpacked, to the end of the copy and
/// points its directory record there.
fn put_in(dir: &Path, data: &mut GameData, folder: &str, name: &str, bytes: &[u8]) {
    // The directory record of the file, in the folder's extent.
    let levels = data.fs.lookup(folder).unwrap();
    let mut iso = File::options()
        .read(true)
        .write(true)
        .open(dir.join("data.iso"))
        .expect("data.iso: run make first");
    let mut extent = vec![0u8; (levels.size as usize).div_ceil(SECTOR_SIZE) * SECTOR_SIZE];
    let dir_at = levels.lba as u64 * SECTOR_SIZE as u64;
    iso.seek(SeekFrom::Start(dir_at)).unwrap();
    iso.read_exact(&mut extent).unwrap();
    let name = format!("{name};1");
    let pos = extent
        .windows(name.len())
        .position(|w| w == name.as_bytes())
        .expect("directory record");
    let rec = pos - 33;
    // Append the bytes, sector-aligned.
    let end = iso.seek(SeekFrom::End(0)).unwrap();
    let lba = end.div_ceil(SECTOR_SIZE as u64) as u32;
    let mut padded = bytes.to_vec();
    padded.resize(padded.len().div_ceil(SECTOR_SIZE) * SECTOR_SIZE, 0);
    iso.seek(SeekFrom::Start(lba as u64 * SECTOR_SIZE as u64))
        .unwrap();
    iso.write_all(&padded).unwrap();
    let sectors = lba + (padded.len() / SECTOR_SIZE) as u32;
    // Both-endian extent and size.
    let both = |v: u32| {
        let mut b = [0u8; 8];
        b[..4].copy_from_slice(&v.to_le_bytes());
        b[4..].copy_from_slice(&v.to_be_bytes());
        b
    };
    extent[rec + 2..rec + 10].copy_from_slice(&both(lba));
    extent[rec + 10..rec + 18].copy_from_slice(&both(bytes.len() as u32));
    iso.seek(SeekFrom::Start(dir_at)).unwrap();
    iso.write_all(&extent).unwrap();
    // The volume's size in the primary volume descriptor.
    iso.seek(SeekFrom::Start(16 * SECTOR_SIZE as u64 + 80))
        .unwrap();
    iso.write_all(&both(sectors)).unwrap();
    println!(
        "{}: {} bytes unpacked at sector {lba}",
        &name[..name.len() - 2],
        bytes.len()
    );
}

fn make(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let data_dir = locate_data_dir(None);
    let disc = Disc::open_dir(&data_dir).expect("game CD");
    let mut reader = disc.data_reader().expect("data track");
    let mut out = File::create(dir.join("data.iso")).unwrap();
    let mut buf = vec![0u8; SECTOR_SIZE * 256];
    let total = reader.sector_count();
    let mut lba = 0;
    while lba < total {
        let n = (total - lba).min(256);
        let b = &mut buf[..n as usize * SECTOR_SIZE];
        reader.read_sectors(lba, b).unwrap();
        out.write_all(b).unwrap();
        lba += n;
    }
    // The original sheet with the data track pointed at the copy.
    let cue_path = std::fs::read_dir(&data_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cue")))
        .unwrap();
    let cue = std::fs::read_to_string(&cue_path).unwrap();
    let mut lines = cue.lines();
    let file_line = lines.next().unwrap();
    let bin = file_line.split('"').nth(1).unwrap();
    let bin = std::fs::canonicalize(data_dir.join(bin)).unwrap();
    let bin = bin
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let mut sheet =
        String::from("FILE \"data.iso\" BINARY\n  TRACK 01 MODE1/2048\n    INDEX 01 00:00:00\n");
    sheet += &format!("FILE \"{bin}\" BINARY\n");
    let mut skipping = true;
    for l in lines {
        if l.trim_start().starts_with("TRACK 02") {
            skipping = false;
        }
        if !skipping {
            sheet += l;
            sheet += "\n";
        }
    }
    std::fs::write(dir.join("patched.cue"), sheet).unwrap();
    println!("{} sectors -> {}", total, dir.join("data.iso").display());
}

/// `cdcopy mark DIR SEA=n[:W] LAND=n[:W] SKY=n ...`: paints calibration marks into those
/// `GFX` textures of the copy, so that their scale, tiling and offset can be
/// read off screenshots of the original and of ours alike. A sea or land gets a red
/// border round each W×W tile (128 unless given) and a white square in its
/// first corner, and tile `k` of a strip of them `k` small squares beside
/// it (so animation frames can be told apart); a 1024-wide sky a white stripe every 64 columns, column
/// `64k` with a stripe `8 + 3k` rows long. `cdcopy unmark DIR SEA=n ...`
/// puts the original files back.
fn mark(dir: &Path, files: &[String], marked: bool) {
    let mut data = GameData::open(&locate_data_dir(None)).expect("game CD");
    let pal = data.palette("GFX/LM3D.PAL").unwrap();
    let nearest = |c: [i32; 3]| {
        (1..256)
            .min_by_key(|&i| {
                let p = pal[i];
                (0..3).map(|k| (p[k] as i32 - c[k]).pow(2)).sum::<i32>()
            })
            .unwrap() as u8
    };
    let (red, white) = (nearest([255, 0, 0]), nearest([255, 255, 255]));
    for f in files {
        let (kind, n) = f.split_once('=').expect("KIND=n[:WIDTH]");
        let (n, width) = n.split_once(':').unwrap_or((n, ""));
        let n: u8 = n.parse().unwrap();
        let mut px = data.gfx(kind, n).unwrap();
        if marked {
            match kind {
                "SEA" | "LAND" => {
                    let w = width.parse().unwrap_or(128);
                    for (i, p) in px.iter_mut().enumerate() {
                        let (x, y) = (i % w, (i / w) % w);
                        if x < 2 || y < 2 {
                            *p = red;
                        }
                        if (4..20).contains(&x) && (4..20).contains(&y) {
                            *p = white;
                        }
                        // A strip of square frames: frame f also gets f
                        // small squares in a row beside the big one.
                        let frame = i / (w * w);
                        let j = x.saturating_sub(24) / 8;
                        if x >= 24 && j < frame && x % 8 < 5 && (4..9).contains(&y) {
                            *p = white;
                        }
                    }
                }
                "SKY" => {
                    let w = 1024;
                    for (i, p) in px.iter_mut().enumerate() {
                        let (x, y) = (i % w, i / w);
                        if x % 64 < 3 && y < 8 + 3 * (x / 64) {
                            *p = white;
                        }
                    }
                }
                k => panic!("can't mark {k}"),
            }
        }
        put_in(dir, &mut data, "GFX", &format!("{kind}.{n:03}"), &px);
    }
}
