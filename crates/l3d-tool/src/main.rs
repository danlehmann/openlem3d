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
    /// Print each non-empty layer of a level's block grid as a map, X across
    /// and Z down. `.` is empty, `#` a full cube, a hex digit a cube with only
    /// those segments (bit 3 = top), and a lowercase letter a non-cube shape
    /// (`a` = shape 1, `b` = shape 2, ...). Rows that are empty are left out.
    LevelMap {
        number: u32,
        /// Show cells whose block is flagged steel as `S`, whatever their shape.
        #[arg(long)]
        steel: bool,
        /// Show each non-empty cell's block id instead: `0`-`9`, `a`-`z`
        /// (10-35), `A`-`Z` (36-61), `*` (62, 63).
        #[arg(long, conflicts_with = "steel")]
        ids: bool,
    },
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
    /// Crop the same rectangle from a series of screenshots and tile the
    /// crops into one labelled contact sheet. Works on local PNG files; no CD
    /// image needed.
    ImgMontage {
        #[arg(required = true)]
        shots: Vec<PathBuf>,
        /// Rectangle to crop, as X,Y,W,H.
        #[arg(long)]
        rect: String,
        /// Crops per row.
        #[arg(long, default_value_t = 8)]
        per_row: usize,
        /// Integer upscaling factor.
        #[arg(long, default_value_t = 1)]
        scale: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// For a series of screenshots, print where the content of a rectangle
    /// changes: each run of frames whose crops match is one line, labelled
    /// with the first frame that showed matching content. Works on local PNG
    /// files; no CD image needed.
    ImgChanges {
        #[arg(required = true)]
        shots: Vec<PathBuf>,
        /// Rectangle to compare, as X,Y,W,H.
        #[arg(long)]
        rect: String,
        /// Largest mean absolute difference per colour channel at which two
        /// crops still match.
        #[arg(long, default_value_t = 0.0)]
        tolerance: f64,
    },
    /// For each screenshot, print the connected blobs of marked pixels inside a
    /// rectangle: bounding box, pixel count and centroid. Without
    /// --background a pixel is marked when it is "lemming blue" (blue exceeds
    /// both red and green by at least --margin); with it, when its summed RGB
    /// difference from the background image exceeds --margin. Works on local
    /// PNG files; no CD image needed.
    ImgBlobs {
        #[arg(required = true)]
        shots: Vec<PathBuf>,
        /// Rectangle to search, as X,Y,W,H.
        #[arg(long)]
        rect: String,
        #[arg(long, default_value_t = 60)]
        margin: i32,
        /// Smallest blob, in pixels, that is printed.
        #[arg(long, default_value_t = 6)]
        min_size: usize,
        #[arg(long)]
        background: Option<PathBuf>,
    },
    /// Decode an MHC sprite file and write its cells as a PNG contact sheet,
    /// printing the manifest summary.
    Mhc {
        #[arg(default_value = "LEMM/LEMM.MHC")]
        path: String,
        /// Cell size in pixels (64 for LEMM.MHC, 128 for LEMM128.MHC).
        #[arg(long, default_value_t = 64)]
        size: usize,
        /// Cells to include, as FIRST..END. Defaults to all cells.
        #[arg(long)]
        cells: Option<String>,
        /// Cells per row.
        #[arg(long, default_value_t = 16)]
        per_row: usize,
        /// Integer upscaling factor.
        #[arg(long, default_value_t = 1)]
        scale: usize,
        /// Separate the cells with a gap and print each cell's index above it.
        #[arg(long)]
        labels: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// For each cell of an MHC sprite file, print the other cell that is
    /// closest to its mirror image, allowing a small horizontal shift.
    MhcMirrors {
        #[arg(default_value = "LEMM/LEMM.MHC")]
        path: String,
        /// Cell size in pixels (64 for LEMM.MHC, 128 for LEMM128.MHC).
        #[arg(long, default_value_t = 64)]
        size: usize,
        /// Cells to compare, as FIRST..END. Defaults to all cells.
        #[arg(long)]
        cells: Option<String>,
        /// Only print pairs whose mean absolute difference is at most this.
        #[arg(long, default_value_t = 255.0)]
        max_diff: f64,
    },
    /// Rank the cells of an MHC sprite file by how closely each one matches a
    /// sprite in screenshots. Cells are tried as stored and mirrored, and
    /// compared on their opaque pixels.
    MhcMatch {
        /// Screenshots (PNG), each ranked separately.
        #[arg(required = true)]
        shots: Vec<PathBuf>,
        /// Without --scale: the sprite's tight bounding box, as X,Y,W,H; each
        /// cell's opaque bounding box is stretched onto it. With --scale: the
        /// area to search.
        #[arg(long)]
        rect: String,
        /// Draw cells at this scale and search every position in --rect. A
        /// range MIN..MAX tries scales from MIN to MAX in steps of 3%.
        #[arg(long)]
        scale: Option<String>,
        /// Cells to try, as FIRST..END. Defaults to all cells.
        #[arg(long)]
        cells: Option<String>,
        #[arg(long, default_value = "LEMM/LEMM.MHC")]
        path: String,
        /// Cell size in pixels (64 for LEMM.MHC, 128 for LEMM128.MHC).
        #[arg(long, default_value_t = 64)]
        size: usize,
        /// Largest shift, in screenshot pixels, tried for each rectangle edge.
        #[arg(long, default_value_t = 2)]
        jitter: i32,
        /// Number of best matches to print.
        #[arg(long, default_value_t = 10)]
        top: usize,
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
    write_rgb_png(path, width, &rgb)
}

fn write_rgb_png(path: &Path, width: u32, rgb: &[u8]) -> Result<()> {
    let height = rgb.len() as u32 / (width * 3);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), width, height);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header()?.write_image_data(&rgb[..(width * height * 3) as usize])?;
    Ok(())
}

/// 3×5 bitmaps of the digits 0–9; each row is 3 bits, most significant bit
/// leftmost.
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

/// An RGB8 image with a fixed width.
struct Canvas {
    width: usize,
    rgb: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize, fill: [u8; 3]) -> Self {
        Canvas { width, rgb: fill.repeat(width * height) }
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                let i = (yy * self.width + xx) * 3;
                self.rgb[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    /// Draws `n` in decimal with its top-left corner at (x, y), each font
    /// pixel `px` canvas pixels wide.
    fn number(&mut self, x: usize, y: usize, n: usize, px: usize, c: [u8; 3]) {
        for (k, d) in n.to_string().bytes().enumerate() {
            for (row, bits) in DIGITS[(d - b'0') as usize].iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        self.fill_rect(x + (k * 4 + col) * px, y + row * px, px, px, c);
                    }
                }
            }
        }
    }
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

/// Parses X,Y,W,H.
fn parse_rect(s: &str) -> Result<[i32; 4]> {
    let v: Vec<i32> = s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>().context("rect must be X,Y,W,H")?;
    v.try_into().ok().filter(|r: &[i32; 4]| r[2] > 0 && r[3] > 0).context("rect must be X,Y,W,H with W, H > 0")
}

/// The RGB8 pixels of rectangle X,Y,W,H of an image; pixels outside the
/// image are black.
fn crop_rgb(img: &(u32, u32, Vec<u8>), [x, y, w, h]: [i32; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for yy in y..y + h {
        for xx in x..x + w {
            if (0..img.0 as i32).contains(&xx) && (0..img.1 as i32).contains(&yy) {
                let i = ((yy as u32 * img.0 + xx as u32) * 3) as usize;
                out.extend_from_slice(&img.2[i..i + 3]);
            } else {
                out.extend_from_slice(&[0, 0, 0]);
            }
        }
    }
    out
}

fn parse_range(s: &str) -> Result<std::ops::Range<i32>> {
    let (a, b) = s.split_once("..").context("range must be FIRST..END")?;
    Ok(a.parse()?..b.parse()?)
}

/// The cell indices selected by an optional FIRST..END argument, clamped to
/// `0..len`; all cells when absent.
fn cell_range(arg: Option<&str>, len: usize) -> Result<std::ops::Range<usize>> {
    Ok(match arg {
        Some(s) => {
            let r = parse_range(s)?;
            (r.start.max(0) as usize).min(len)..(r.end.max(0) as usize).min(len)
        }
        None => 0..len,
    })
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

/// How a cell's opaque bounding box is laid onto a screenshot.
enum Placement {
    /// Stretched onto the rectangle (X,Y,W,H) with each edge shifted by up to
    /// `jitter` pixels.
    Fit { rect: [i32; 4], jitter: i32 },
    /// Scaled by each of `scales` and moved to every position where it lies
    /// inside the rectangle (X,Y,W,H).
    Search { rect: [i32; 4], scales: Vec<f64> },
}

/// The scales named by a `--scale` argument: one number, or MIN..MAX meaning
/// MIN, MIN × 1.03, … up to MAX.
fn parse_scales(s: &str) -> Result<Vec<f64>> {
    let Some((a, b)) = s.split_once("..") else {
        return Ok(vec![s.parse().context("scale must be a number or MIN..MAX")?]);
    };
    let (min, max): (f64, f64) = (a.parse()?, b.parse()?);
    anyhow::ensure!(min > 0.0 && min <= max, "scale range must be MIN..MAX with 0 < MIN <= MAX");
    Ok(std::iter::successors(Some(min), |s| Some(s * 1.03)).take_while(|s| *s <= max * 1.0001).collect())
}

/// How far cell `b`, mirrored left to right and shifted by up to 4 pixels,
/// is from cell `a`: the mean absolute RGB difference over the pixels opaque
/// in either cell (an opaque pixel against a transparent one counts as 255),
/// and the shift giving it. `None` if both cells are empty.
fn mirror_diff(a: &[u8], b: &[u8], size: usize, pal: &[[u8; 3]]) -> Option<(f64, i32)> {
    (-4..=4)
        .filter_map(|dx: i32| {
            let (mut sum, mut opaque) = (0u64, 0u64);
            for y in 0..size {
                for x in 0..size {
                    let mx = size as i32 - 1 - x as i32 + dx;
                    let pb = if (0..size as i32).contains(&mx) { b[y * size + mx as usize] } else { 0 };
                    let pa = a[y * size + x];
                    match (pa, pb) {
                        (0, 0) => continue,
                        (0, _) | (_, 0) => sum += 255,
                        _ => {
                            let (ca, cb) = (pal[pa as usize], pal[pb as usize]);
                            sum += (0..3).map(|k| u64::from(ca[k].abs_diff(cb[k]))).sum::<u64>() / 3;
                        }
                    }
                    opaque += 1;
                }
            }
            (opaque > 0).then(|| (sum as f64 / opaque as f64, dx))
        })
        .min_by(|p, q| p.0.total_cmp(&q.0))
}

/// A cell's best fit to a screenshot: mean absolute RGB difference over the
/// cell's opaque pixels, whether the cell was mirrored, the screenshot
/// position of the cell's opaque bounding box, and the box's size there.
struct CellFit {
    score: f64,
    mirrored: bool,
    at: (i32, i32),
    size: (i32, i32),
}

/// Best (lowest-scoring) fit of a cell to the screenshot over all mirrorings
/// and positions the placement allows; `None` if the cell is empty or never
/// fits.
fn best_cell_fit(pixels: &[u8], size: usize, pal: &[[u8; 3]], img: &(u32, u32, Vec<u8>), placement: &Placement) -> Option<CellFit> {
    let opaque: Vec<(usize, usize)> = (0..size * size).filter(|&k| pixels[k] != 0).map(|k| (k % size, k / size)).collect();
    let (bx0, by0) = (opaque.iter().map(|p| p.0).min()?, opaque.iter().map(|p| p.1).min()?);
    let (bx1, by1) = (opaque.iter().map(|p| p.0).max()? + 1, opaque.iter().map(|p| p.1).max()? + 1);
    let (bw, bh) = ((bx1 - bx0) as f64, (by1 - by0) as f64);
    // Compare at most about 400 evenly spread opaque pixels.
    let step = opaque.len().div_ceil(400);
    let samples: Vec<(usize, usize)> = opaque.into_iter().step_by(step).collect();
    // Candidate placements as (x0, y0, x scale, y scale).
    let mut candidates: Vec<(i32, i32, f64, f64)> = Vec::new();
    match *placement {
        Placement::Fit { rect: [x, y, w, h], jitter } => {
            for dx0 in -jitter..=jitter {
                for dx1 in -jitter..=jitter {
                    for dy0 in -jitter..=jitter {
                        for dy1 in -jitter..=jitter {
                            let (cw, ch) = (w + dx1 - dx0, h + dy1 - dy0);
                            if cw > 0 && ch > 0 {
                                candidates.push((x + dx0, y + dy0, cw as f64 / bw, ch as f64 / bh));
                            }
                        }
                    }
                }
            }
        }
        Placement::Search { rect: [x, y, w, h], ref scales } => {
            for &scale in scales {
                let (sw, sh) = ((bw * scale).ceil() as i32, (bh * scale).ceil() as i32);
                for y0 in y..=y + h - sh {
                    for x0 in x..=x + w - sw {
                        candidates.push((x0, y0, scale, scale));
                    }
                }
            }
        }
    }
    let (iw, ih) = (img.0 as i32, img.1 as i32);
    let mut best: Option<CellFit> = None;
    for mirrored in [false, true] {
        for &(x0, y0, sx, sy) in &candidates {
            let mut sum = 0u64;
            for &(cx, cy) in &samples {
                let u = if mirrored { bx1 - 1 - cx } else { cx - bx0 };
                let px = x0 + ((u as f64 + 0.5) * sx) as i32;
                let py = y0 + (((cy - by0) as f64 + 0.5) * sy) as i32;
                let c = pal[pixels[cy * size + cx] as usize];
                let i = ((py.clamp(0, ih - 1) * iw + px.clamp(0, iw - 1)) * 3) as usize;
                sum += (0..3).map(|k| (img.2[i + k] as i32 - c[k] as i32).unsigned_abs() as u64).sum::<u64>();
            }
            let score = sum as f64 / (samples.len() * 3) as f64;
            if best.as_ref().is_none_or(|b| score < b.score) {
                let size = ((bw * sx).round() as i32, (bh * sy).round() as i32);
                best = Some(CellFit { score, mirrored, at: (x0, y0), size });
            }
        }
    }
    best
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
    if let Cmd::ImgMontage { shots, rect, per_row, scale, out } = &cli.cmd {
        let [x, y, w, h] = parse_rect(rect)?;
        let (gap, label) = (2, 8);
        let (cw, ch) = (w as usize * scale + gap, h as usize * scale + gap + label);
        let rows = shots.len().div_ceil(*per_row);
        let mut sheet = Canvas::new(cw * per_row, ch * rows, [40, 40, 40]);
        for (k, shot) in shots.iter().enumerate() {
            let img = read_rgb(shot)?;
            let crop = crop_rgb(&img, [x, y, w, h]);
            let (ox, oy) = ((k % per_row) * cw, (k / per_row) * ch);
            sheet.number(ox + 1, oy + 1, k, 1, [255, 255, 0]);
            for yy in 0..h as usize * scale {
                for xx in 0..w as usize * scale {
                    let i = ((yy / scale) * w as usize + xx / scale) * 3;
                    let c = [crop[i], crop[i + 1], crop[i + 2]];
                    sheet.fill_rect(ox + xx, oy + label + yy, 1, 1, c);
                }
            }
        }
        write_rgb_png(out, sheet.width as u32, &sheet.rgb)?;
        return Ok(());
    }
    if let Cmd::ImgBlobs { shots, rect, margin, min_size, background } = &cli.cmd {
        let [rx, ry, rw, rh] = parse_rect(rect)?;
        let bg = background.as_ref().map(|p| read_rgb(p).map(|img| crop_rgb(&img, [rx, ry, rw, rh]))).transpose()?;
        let (w, h) = (rw as usize, rh as usize);
        for (k, shot) in shots.iter().enumerate() {
            let crop = crop_rgb(&read_rgb(shot)?, [rx, ry, rw, rh]);
            let mut mask: Vec<bool> = crop
                .chunks(3)
                .enumerate()
                .map(|(i, p)| match &bg {
                    Some(bg) => (0..3).map(|c| (p[c] as i32 - bg[i * 3 + c] as i32).abs()).sum::<i32>() > *margin,
                    None => {
                        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
                        b - r >= *margin && b - g >= *margin
                    }
                })
                .collect();
            let mut blobs = Vec::new();
            for start in 0..w * h {
                if !mask[start] {
                    continue;
                }
                mask[start] = false;
                let mut stack = vec![start];
                let (mut x0, mut y0, mut x1, mut y1, mut n, mut sx, mut sy) = (w, h, 0, 0, 0usize, 0usize, 0usize);
                while let Some(i) = stack.pop() {
                    let (x, y) = (i % w, i / w);
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                    (n, sx, sy) = (n + 1, sx + x, sy + y);
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            if (0..w as i32).contains(&nx) && (0..h as i32).contains(&ny) {
                                let j = ny as usize * w + nx as usize;
                                if mask[j] {
                                    mask[j] = false;
                                    stack.push(j);
                                }
                            }
                        }
                    }
                }
                if n >= *min_size {
                    blobs.push((x0, y0, x1, y1, n, sx as f64 / n as f64, sy as f64 / n as f64));
                }
            }
            blobs.sort_by(|a, b| a.5.total_cmp(&b.5));
            let list: Vec<String> = blobs
                .iter()
                .map(|&(x0, y0, x1, y1, n, cx, cy)| {
                    let (ox, oy) = (rx as usize, ry as usize);
                    format!("[{}..{} x {}..{} n{n} c({:.1},{:.1})]", x0 + ox, x1 + ox, y0 + oy, y1 + oy, cx + ox as f64, cy + oy as f64)
                })
                .collect();
            println!("{k:4}: {}", list.join(" "));
        }
        return Ok(());
    }
    if let Cmd::ImgChanges { shots, rect, tolerance } = &cli.cmd {
        let r = parse_rect(rect)?;
        let mean_diff = |a: &[u8], b: &[u8]| a.iter().zip(b).map(|(p, q)| u64::from(p.abs_diff(*q))).sum::<u64>() as f64 / a.len() as f64;
        let mut seen: Vec<(Vec<u8>, usize)> = Vec::new();
        let mut prev: Option<usize> = None;
        for (k, shot) in shots.iter().enumerate() {
            let crop = crop_rgb(&read_rgb(shot)?, r);
            let id = match seen.iter().position(|s| mean_diff(&s.0, &crop) <= *tolerance) {
                Some(i) => i,
                None => {
                    seen.push((crop, k));
                    seen.len() - 1
                }
            };
            if prev != Some(id) {
                println!("frame {k:4}: content #{id} (first seen at frame {})", seen[id].1);
                prev = Some(id);
            }
        }
        return Ok(());
    }
    let dir = data_dir(&cli);
    let disc = Disc::open_dir(&dir).with_context(|| format!("opening CD image in {}", dir.display()))?;
    match &cli.cmd {
        Cmd::ImgShift { .. } | Cmd::ImgFind { .. } | Cmd::ImgMontage { .. } | Cmd::ImgChanges { .. } | Cmd::ImgBlobs { .. } => {
            unreachable!("handled before opening the disc")
        }
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
        Cmd::LevelMap { number, steel, ids } => {
            use l3d_formats::level::{SIZE_X, SIZE_Y, SIZE_Z};
            let mut fs = open_fs(&disc)?;
            let level = load_level(&mut fs, *number)?;
            let blk = load_blk(&mut fs, *number)?;
            let is_steel = |id: u8| blk.defs[id as usize].flags & l3d_formats::blk::flags::STEEL != 0;
            let header: String = (0..SIZE_X).map(|x| char::from_digit((x % 10) as u32, 10).unwrap()).collect();
            for y in 0..SIZE_Y {
                if (0..SIZE_X).all(|x| (0..SIZE_Z).all(|z| level.block(x, y, z).is_empty())) {
                    continue;
                }
                println!("layer y={y}\n    x {header}");
                for z in 0..SIZE_Z {
                    let row: String = (0..SIZE_X)
                        .map(|x| match level.block(x, y, z) {
                            c if c.is_empty() => '.',
                            c if *ids => id_char(c.id),
                            c if *steel && is_steel(c.id) => 'S',
                            c if c.shape != 0 => (b'a' + c.shape - 1) as char,
                            c if c.segments == 0xF => '#',
                            c => char::from_digit(c.segments as u32, 16).unwrap(),
                        })
                        .collect();
                    if row.bytes().any(|b| b != b'.') {
                        println!("z={z:2} {row}");
                    }
                }
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
                // Side neighbours at the same height that are empty cells.
                let open: Vec<&str> = [(1i32, 0i32, "+X"), (-1, 0, "-X"), (0, 1, "+Z"), (0, -1, "-Z")]
                    .iter()
                    .filter(|(dx, dz, _)| {
                        let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                        (0..32).contains(&nx) && (0..32).contains(&nz) && l.block(nx as usize, y, nz as usize).is_empty()
                    })
                    .map(|(_, _, name)| *name)
                    .collect();
                println!(
                    "  special block {} at ({x},{y},{z}) rot {} segs {:04b} open sides {}",
                    b.id,
                    b.rotation,
                    b.segments,
                    open.join(" ")
                );
            }
            for (x, y, z, _, o) in l.cells().filter(|c| (0x60..=0x67).contains(&c.4.kind)) {
                println!("  interactive object {:#04x} at ({x},{y},{z}) extra {:#04x}", o.kind, o.extra);
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
        Cmd::Mhc { path, size, cells, per_row, scale, labels, out } => {
            let mut fs = open_fs(&disc)?;
            let pal = load_palette(&mut fs, "GFX/LM3D.PAL")?;
            let data = rnc::unpack_if_packed(fs.read_path(path)?)?;
            let mhc = l3d_formats::mhc::MhcFile::parse(&data, *size)?;
            // Manifest summary: cells per animation and the flag values seen.
            let mut per_anim: BTreeMap<u8, (usize, BTreeSet<u8>)> = BTreeMap::new();
            for e in &mhc.entries {
                let s = per_anim.entry(e.animation).or_default();
                s.0 += 1;
                s.1.insert(e.flags);
            }
            for (a, (n, flags)) in &per_anim {
                println!("animation {a:3}: {n:3} cells, flags {flags:02x?}");
            }
            let range = cell_range(cells.as_deref(), mhc.entries.len())?;
            anyhow::ensure!(!range.is_empty() && *per_row > 0 && *scale > 0, "nothing to draw");
            // Layout: each slot is a label strip (if any) above the scaled cell,
            // plus a gap to the right and below (if labelled).
            let cell_px = size * scale;
            let (gap, label_h) = if *labels { (4, 14) } else { (0, 0) };
            let (slot_w, slot_h) = (cell_px + gap, label_h + cell_px + gap);
            let rows = range.len().div_ceil(*per_row);
            let background = if *labels { [48, 64, 80] } else { pal[0] };
            let mut sheet = Canvas::new(per_row * slot_w, rows * slot_h, background);
            let mut failed = 0;
            for (k, i) in range.clone().enumerate() {
                let (ox, oy) = ((k % per_row) * slot_w, (k / per_row) * slot_h);
                if *labels {
                    sheet.number(ox + 1, oy + 2, i, 2, [255, 255, 0]);
                }
                match mhc.cell(i) {
                    Ok(cell) => {
                        for y in 0..cell_px {
                            for x in 0..cell_px {
                                let p = cell.pixels[(y / scale) * size + x / scale];
                                if p != 0 || !*labels {
                                    sheet.fill_rect(ox + x, oy + label_h + y, 1, 1, pal[p as usize]);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        eprintln!("{e}");
                    }
                }
            }
            write_rgb_png(out, sheet.width as u32, &sheet.rgb)?;
            println!("{} cells, {failed} failed -> {}", range.len(), out.display());
        }
        Cmd::MhcMirrors { path, size, cells, max_diff } => {
            let mut fs = open_fs(&disc)?;
            let pal = load_palette(&mut fs, "GFX/LM3D.PAL")?;
            let data = rnc::unpack_if_packed(fs.read_path(path)?)?;
            let mhc = l3d_formats::mhc::MhcFile::parse(&data, *size)?;
            let range = cell_range(cells.as_deref(), mhc.entries.len())?;
            let decoded: Vec<(usize, Vec<u8>)> = range.map(|i| Ok((i, mhc.cell(i)?.pixels))).collect::<Result<_>>()?;
            for (i, a) in &decoded {
                let best = decoded
                    .iter()
                    .filter(|(j, _)| j != i)
                    .filter_map(|(j, b)| mirror_diff(a, b, *size, &pal).map(|(d, dx)| (*j, d, dx)))
                    .min_by(|p, q| p.1.total_cmp(&q.1));
                if let Some((j, d, dx)) = best.filter(|b| b.1 <= *max_diff) {
                    println!("cell {i:3} ~ mirrored cell {j:3} shifted {dx:+2}: mean abs diff {d:.1}");
                }
            }
        }
        Cmd::MhcMatch { shots, rect, scale, cells, path, size, jitter, top } => {
            let mut fs = open_fs(&disc)?;
            let pal = load_palette(&mut fs, "GFX/LM3D.PAL")?;
            let data = rnc::unpack_if_packed(fs.read_path(path)?)?;
            let mhc = l3d_formats::mhc::MhcFile::parse(&data, *size)?;
            let r: Vec<i32> = rect.split(',').map(str::parse).collect::<Result<_, _>>().context("rect must be X,Y,W,H")?;
            let rect: [i32; 4] = r.try_into().ok().filter(|r: &[i32; 4]| r[2] > 0 && r[3] > 0).context("rect must be X,Y,W,H")?;
            let placement = match scale {
                Some(scale) => Placement::Search { rect, scales: parse_scales(scale)? },
                None => Placement::Fit { rect, jitter: *jitter },
            };
            let range = cell_range(cells.as_deref(), mhc.entries.len())?;
            let decoded: Vec<(usize, Vec<u8>)> = range.map(|i| Ok((i, mhc.cell(i)?.pixels))).collect::<Result<_>>()?;
            for shot in shots {
                let img = read_rgb(shot)?;
                let mut results: Vec<(usize, CellFit)> = decoded
                    .iter()
                    .filter_map(|(i, px)| best_cell_fit(px, *size, &pal, &img, &placement).map(|f| (*i, f)))
                    .collect();
                results.sort_by(|a, b| a.1.score.total_cmp(&b.1.score));
                println!("{}:", shot.display());
                for (i, f) in results.iter().take(*top) {
                    let m = if f.mirrored { " mirrored" } else { "" };
                    let (x, y, (w, h)) = (f.at.0, f.at.1, f.size);
                    println!("  cell {i:3}{m:9} at {x:4},{y:4} size {w:3}x{h:3}: mean abs diff {:.1}", f.score);
                }
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

/// One character for a block id: `0`-`9`, `a`-`z` (10-35), `A`-`Z` (36-61),
/// `*` for 62 and 63.
fn id_char(id: u8) -> char {
    match id {
        0..=9 => (b'0' + id) as char,
        10..=35 => (b'a' + id - 10) as char,
        36..=61 => (b'A' + id - 36) as char,
        _ => '*',
    }
}
