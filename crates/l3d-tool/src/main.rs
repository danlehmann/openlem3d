//! `l3d-tool`: inspect and extract Lemmings 3D game data from a CD image.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use l3d_formats::{blk::BlockSet, disc::Disc, iso9660::IsoFs, level::Level, rnc};

#[derive(Parser)]
struct Cli {
    /// Directory containing the CD image (.cue + .bin). Defaults to
    /// $OPENLEM3D_DATA, then ./gamedata.
    #[arg(long, global = true)]
    data: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List the disc's tracks.
    Tracks,
    /// List all files on the data track.
    Ls,
    /// Extract files whose path starts with PREFIX (case-insensitive).
    Extract {
        prefix: String,
        #[arg(long)]
        out: PathBuf,
        /// Decompress RNC-packed files.
        #[arg(long)]
        unpack: bool,
    },
    /// Decompress every RNC-packed file and report the results.
    RncCheck,
    /// Print a summary line for every level.
    Levels,
    /// Print a level's header fields and block statistics.
    Level { number: u32 },
    /// For each level, count block ids used by the grid that are placeholders
    /// in BLK.<level number> versus BLK.<texture set>.
    BlkMatch,
    /// Print statistics about a palette file.
    PalInfo {
        #[arg(default_value = "GFX/LM3D.PAL")]
        path: String,
    },
    /// Find the horizontal shift (in pixels) that best aligns a region of
    /// image B with image A. Works on local PNG files; no CD image needed.
    ImgShift {
        a: PathBuf,
        b: PathBuf,
        /// Rows to compare, as FIRST..END.
        #[arg(long)]
        rows: String,
        /// Columns of A to compare, as FIRST..END.
        #[arg(long)]
        cols: String,
        /// Largest shift to try in either direction.
        #[arg(long, default_value_t = 200)]
        max: i32,
    },
    /// Find where a region of image A appears in image B at 1:1 scale
    /// (B wraps horizontally). Works on local PNG files; no CD image needed.
    ImgFind {
        a: PathBuf,
        b: PathBuf,
        /// Rows of A, as FIRST..END.
        #[arg(long)]
        rows: String,
        /// Columns of A, as FIRST..END.
        #[arg(long)]
        cols: String,
        /// Horizontal scale applied to B (nearest-neighbour).
        #[arg(long, default_value_t = 1.0)]
        sx: f64,
        /// Vertical scale applied to B (nearest-neighbour).
        #[arg(long, default_value_t = 1.0)]
        sy: f64,
    },
    /// Render a raw 8-bit indexed image file from the disc to PNG.
    Png {
        path: String,
        /// Image width in pixels.
        #[arg(long)]
        width: u32,
        #[arg(long, default_value = "GFX/LM3D.PAL")]
        palette: String,
        /// Bytes to skip before the pixel data.
        #[arg(long, default_value_t = 0)]
        skip: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

/// Reads a 256-entry 6-bit-per-channel VGA palette and scales it to 8 bits.
fn load_palette(fs: &mut IsoFs, path: &str) -> Result<Vec<[u8; 3]>> {
    let raw = fs.read_path(path)?;
    anyhow::ensure!(raw.len() == 768, "palette is {} bytes", raw.len());
    Ok(raw.as_chunks::<3>().0.iter().map(|c| [c[0], c[1], c[2]].map(|v| (v << 2) | (v >> 4))).collect())
}

fn write_png(path: &Path, width: u32, pixels: &[u8], pal: &[[u8; 3]]) -> Result<()> {
    let height = pixels.len() as u32 / width;
    let rgb: Vec<u8> = pixels[..(width * height) as usize].iter().flat_map(|&i| pal[i as usize]).collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), width, height);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header()?.write_image_data(&rgb)?;
    Ok(())
}

fn load_level(fs: &mut IsoFs, n: u32) -> Result<Level> {
    let raw = fs.read_path(&format!("LEVELS/LEVEL.{n:03}"))?;
    Ok(Level::parse(&rnc::unpack_if_packed(raw)?)?)
}

fn load_blk(fs: &mut IsoFs, n: u32) -> Result<BlockSet> {
    Ok(BlockSet::parse(&fs.read_path(&format!("LEVELS/BLK.{n:03}"))?)?)
}

/// Block ids that occur in the level's non-empty cells.
fn used_ids(level: &Level) -> BTreeSet<u8> {
    level.cells().filter(|c| !c.3.is_empty()).map(|c| c.3.id).collect()
}

fn data_dir(cli: &Cli) -> PathBuf {
    cli.data
        .clone()
        .or_else(|| std::env::var_os("OPENLEM3D_DATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("gamedata"))
}

fn open_fs(disc: &Disc) -> Result<IsoFs> {
    Ok(IsoFs::new(disc.data_reader()?)?)
}

/// Decodes a PNG into (width, height, RGB8 pixels).
fn read_rgb(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let ch = info.color_type.samples();
    let rgb = buf[..info.buffer_size()]
        .chunks(ch)
        .flat_map(|p| if ch < 3 { [p[0]; 3] } else { [p[0], p[1], p[2]] })
        .collect();
    Ok((info.width, info.height, rgb))
}

fn parse_range(s: &str) -> Result<std::ops::Range<i32>> {
    let (a, b) = s.split_once("..").context("range must be FIRST..END")?;
    Ok(a.parse()?..b.parse()?)
}

/// Mean absolute RGB difference between `a[x, y]` and `b[x + dx, y]` over the
/// region, or `None` if the shifted region leaves image B.
fn region_diff(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>), rows: &std::ops::Range<i32>, cols: &std::ops::Range<i32>, dx: i32) -> Option<f64> {
    let (mut sum, mut n) = (0u64, 0u64);
    for y in rows.clone() {
        for x in cols.clone() {
            let bx = x + dx;
            if bx < 0 || bx >= b.0 as i32 || y >= a.1 as i32 || y >= b.1 as i32 || x >= a.0 as i32 {
                return None;
            }
            let ia = ((y as u32 * a.0 + x as u32) * 3) as usize;
            let ib = ((y as u32 * b.0 + bx as u32) * 3) as usize;
            for c in 0..3 {
                sum += (a.2[ia + c] as i32 - b.2[ib + c] as i32).unsigned_abs() as u64;
            }
            n += 3;
        }
    }
    Some(sum as f64 / n as f64)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Cmd::ImgShift { a, b, rows, cols, max } = &cli.cmd {
        let (ia, ib) = (read_rgb(a)?, read_rgb(b)?);
        let (rows, cols) = (parse_range(rows)?, parse_range(cols)?);
        let mut scores: Vec<(i32, f64)> =
            (-max..=*max).filter_map(|dx| region_diff(&ia, &ib, &rows, &cols, dx).map(|d| (dx, d))).collect();
        scores.sort_by(|p, q| p.1.total_cmp(&q.1));
        for (dx, d) in scores.iter().take(5) {
            println!("shift {dx:+4}: mean abs diff {d:.2}");
        }
        return Ok(());
    }
    if let Cmd::ImgFind { a, b, rows, cols, sx, sy } = &cli.cmd {
        let (ia, src) = (read_rgb(a)?, read_rgb(b)?);
        let (rows, cols) = (parse_range(rows)?, parse_range(cols)?);
        let ib = {
            let (w, h) = ((src.0 as f64 * sx).round() as u32, (src.1 as f64 * sy).round() as u32);
            let mut px = Vec::with_capacity((w * h * 3) as usize);
            for y in 0..h {
                for x in 0..w {
                    let (u, v) = (((x as f64 / sx) as u32).min(src.0 - 1), ((y as f64 / sy) as u32).min(src.1 - 1));
                    let i = ((v * src.0 + u) * 3) as usize;
                    px.extend_from_slice(&src.2[i..i + 3]);
                }
            }
            (w, h, px)
        };
        let (bw, bh) = (ib.0 as i32, ib.1 as i32);
        let px = |img: &(u32, u32, Vec<u8>), x: i32, y: i32| {
            let i = ((y as u32 * img.0 + x as u32) * 3) as usize;
            [img.2[i] as i32, img.2[i + 1] as i32, img.2[i + 2] as i32]
        };
        let mut best: Vec<(f64, i32, i32)> = Vec::new();
        for oy in 0..=(bh - rows.len() as i32) {
            for ox in 0..bw {
                let mut sum = 0i64;
                for y in rows.clone() {
                    for x in cols.clone() {
                        let p = px(&ia, x, y);
                        let q = px(&ib, (ox + x - cols.start).rem_euclid(bw), oy + y - rows.start);
                        sum += (0..3).map(|c| (p[c] - q[c]).abs() as i64).sum::<i64>();
                    }
                }
                best.push((sum as f64 / (rows.len() * cols.len() * 3) as f64, ox, oy));
            }
        }
        best.sort_by(|p, q| p.0.total_cmp(&q.0));
        for (d, x, y) in best.iter().take(5) {
            println!("A({},{}) ~ B({x},{y}): mean abs diff {d:.2}", cols.start, rows.start);
        }
        return Ok(());
    }
    let dir = data_dir(&cli);
    let disc = Disc::open_dir(&dir).with_context(|| format!("opening CD image in {}", dir.display()))?;
    match &cli.cmd {
        Cmd::ImgShift { .. } | Cmd::ImgFind { .. } => unreachable!("handled before opening the disc"),
        Cmd::Tracks => {
            for t in &disc.tracks {
                println!("{:2} {:?} {} sectors ({:.1}s) at byte {}", t.number, t.mode, t.sectors, t.seconds(), t.byte_offset);
            }
        }
        Cmd::Ls => {
            let mut fs = open_fs(&disc)?;
            println!("volume: {}", fs.volume_id);
            for (path, e) in fs.walk()? {
                if e.is_dir {
                    println!("{:>10}  {path}/", "");
                } else {
                    println!("{:>10}  {path}", e.size);
                }
            }
        }
        Cmd::Extract { prefix, out, unpack } => {
            let mut fs = open_fs(&disc)?;
            let prefix = prefix.to_ascii_uppercase();
            for (path, e) in fs.walk()? {
                if e.is_dir || !path.to_ascii_uppercase().starts_with(&prefix) {
                    continue;
                }
                let mut data = fs.read_file(&e)?;
                if *unpack && rnc::is_packed(&data) {
                    data = rnc::unpack(&data).with_context(|| format!("unpacking {path}"))?;
                }
                let dst = out.join(Path::new(&path));
                std::fs::create_dir_all(dst.parent().unwrap())?;
                std::fs::write(&dst, &data)?;
                println!("{path} ({} bytes)", data.len());
            }
        }
        Cmd::RncCheck => {
            let mut fs = open_fs(&disc)?;
            let (mut ok, mut bad) = (0, 0);
            for (path, e) in fs.walk()? {
                if e.is_dir {
                    continue;
                }
                let data = fs.read_file(&e)?;
                if !rnc::is_packed(&data) {
                    continue;
                }
                let h = rnc::parse_header(&data)?;
                match rnc::unpack(&data) {
                    Ok(u) => {
                        ok += 1;
                        println!("ok   {path}: {} -> {} (blocks {}, leeway {})", data.len(), u.len(), h.blocks, h.leeway);
                    }
                    Err(err) => {
                        bad += 1;
                        println!("FAIL {path}: {err} ({h:?})");
                    }
                }
            }
            println!("{ok} ok, {bad} failed");
        }
        Cmd::Levels => {
            let mut fs = open_fs(&disc)?;
            println!("num title                            tex land obj sgn sea ani trp sky wal thm mus lem save time  rr  flags");
            for n in 0..100 {
                let l = load_level(&mut fs, n)?;
                println!(
                    "{n:03} {:32} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:4} {:2}:{:02} {:3} {:04x}",
                    l.title, l.texture_set, l.land_gfx, l.object_set, l.sign_set, l.sea_gfx, l.anim_object,
                    l.trap_type, l.sky_gfx, l.walls_set, l.theme, l.music, l.lemmings, l.save_requirement,
                    l.time_minutes, l.time_seconds, l.release_rate, l.flags
                );
            }
        }
        Cmd::Level { number } => {
            let mut fs = open_fs(&disc)?;
            let l = load_level(&mut fs, *number)?;
            let blk = load_blk(&mut fs, *number)?;
            println!("title: {:?}  comment: {:?}", l.title, l.comment);
            println!("skills: {:?}", l.skills);
            println!("land polygons: {:?}", l.land_polygons);
            println!("cameras: {:?}", l.cameras);
            println!("border kill {:?} ceiling {} pivot {:?} flags2 {:02x}", l.border_kill, l.ceiling_kill, l.preview_pivot, l.flags2);
            let mut counts: BTreeMap<(u8, u8), usize> = BTreeMap::new();
            let mut objs: BTreeMap<u8, usize> = BTreeMap::new();
            for (_, _, _, b, o) in l.cells() {
                if !b.is_empty() {
                    *counts.entry((b.id, b.shape)).or_default() += 1;
                }
                if o.kind != 0 {
                    *objs.entry(o.kind).or_default() += 1;
                }
            }
            println!("(block id, shape) -> count, def:");
            for ((id, shape), c) in counts {
                let d = &blk.defs[id as usize];
                let tex: Vec<u8> = d.faces.iter().map(|f| f.texture).collect();
                println!("  ({id:2},{shape:2}) x{c:4}  flags {:02x} tex {tex:?}", d.flags);
            }
            println!("object kinds: {objs:?}");
            for (x, y, z, b, _) in l.cells().filter(|c| !c.3.is_empty() && c.3.id <= 2) {
                println!("  special block {} at ({x},{y},{z}) rot {} segs {:04b}", b.id, b.rotation, b.segments);
            }
            println!("header:");
            for (i, row) in l.header.chunks(16).enumerate() {
                println!("  {:04x}: {}", i * 16, row.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
            }
        }
        Cmd::BlkMatch => {
            let mut fs = open_fs(&disc)?;
            let (mut own_bad, mut tex_bad) = (0, 0);
            for n in 0..100 {
                let l = load_level(&mut fs, n)?;
                let used = used_ids(&l);
                let own = load_blk(&mut fs, n)?;
                let own_missing_ids: Vec<u8> =
                    used.iter().copied().filter(|&i| own.defs[i as usize].is_placeholder()).collect();
                let own_missing = own_missing_ids.len();
                let tex_missing = match load_blk(&mut fs, l.texture_set as u32) {
                    Ok(b) => used.iter().filter(|&&i| b.defs[i as usize].is_placeholder()).count().to_string(),
                    Err(_) => "n/a".into(),
                };
                own_bad += (own_missing > 0) as u32;
                tex_bad += (tex_missing != "0") as u32;
                println!("{n:03} tex {:3} used {:2}  missing in own BLK: {own_missing_ids:?}  in BLK.<tex>: {tex_missing}", l.texture_set, used.len());
            }
            println!("levels with undefined used ids: own {own_bad}, by texture set {tex_bad}");
        }
        Cmd::PalInfo { path } => {
            let mut fs = open_fs(&disc)?;
            let raw = fs.read_path(path)?;
            println!("{} bytes, max component {}", raw.len(), raw.iter().max().unwrap_or(&0));
            for (i, c) in raw.chunks(3).enumerate().filter(|(i, _)| i % 16 == 0 || *i < 16) {
                println!("  {i:3}: {c:?}");
            }
        }
        Cmd::Png { path, width, palette, skip, out } => {
            let mut fs = open_fs(&disc)?;
            let pal = load_palette(&mut fs, palette)?;
            let data = rnc::unpack_if_packed(fs.read_path(path)?)?;
            write_png(out, *width, &data[*skip..], &pal)?;
            println!("{} bytes -> {}x{}", data.len() - skip, width, (data.len() - skip) as u32 / width);
        }
    }
    Ok(())
}
