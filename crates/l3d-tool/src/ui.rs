//! `l3d-tool ui`: renders the user-interface graphics as PNG contact sheets.

use std::path::Path;

use anyhow::{Context, Result};
use l3d_formats::image::{IndexedImage, Palette, vga_palette};
use l3d_formats::iso9660::IsoFs;
use l3d_formats::{font::Font, icons::Icons, rnc, screen, sheets, title};

/// Names accepted by `l3d-tool ui`, in rendering order.
pub const PARTS: [&str; 10] = [
    "title-mhc",
    "title-menu",
    "title-font",
    "lemmings-font",
    "icons",
    "sheets",
    "loading",
    "intro",
    "scene",
    "bmps",
];

/// Background of contact sheets, chosen to stand apart from the game colours.
const BACKDROP: [u8; 3] = [64, 0, 64];

fn read(fs: &mut IsoFs, path: &str) -> Result<Vec<u8>> {
    Ok(rnc::unpack_if_packed(
        fs.read_path(path)
            .with_context(|| format!("reading {path}"))?,
    )?)
}

pub fn palette(fs: &mut IsoFs, path: &str) -> Result<Palette> {
    Ok(vga_palette(&read(fs, path)?)?)
}

/// Lays `cells` out `per_row` across, each slot as large as the largest
/// cell, `scale`× enlarged, with a 1-pixel (scaled) gap; draws index 0 as
/// the backdrop when `transparent`, and writes the result to `out`.
fn contact(
    out: &Path,
    cells: &[IndexedImage],
    per_row: usize,
    scale: usize,
    pal: &Palette,
    transparent: bool,
) -> Result<()> {
    anyhow::ensure!(!cells.is_empty(), "nothing to draw");
    let cw = cells.iter().map(|c| c.width).max().unwrap_or(0) + 1;
    let ch = cells.iter().map(|c| c.height).max().unwrap_or(0) + 1;
    let per_row = per_row.min(cells.len());
    let rows = cells.len().div_ceil(per_row);
    let (w, h) = (per_row * cw * scale, rows * ch * scale);
    let mut rgb = BACKDROP.repeat(w * h);
    for (k, c) in cells.iter().enumerate() {
        let (ox, oy) = ((k % per_row) * cw * scale, (k / per_row) * ch * scale);
        for y in 0..c.height * scale {
            for x in 0..c.width * scale {
                let p = c.get(x / scale, y / scale);
                if p == 0 && transparent {
                    continue;
                }
                let i = ((oy + y) * w + ox + x) * 3;
                rgb[i..i + 3].copy_from_slice(&pal[p as usize]);
            }
        }
    }
    crate::write_rgb_png(out, w as u32, &rgb)?;
    println!("{}", out.display());
    Ok(())
}

fn font_sheet(out: &Path, font: &Font, scale: usize, pal: &Palette) -> Result<()> {
    contact(out, &font.glyphs, 16, scale, pal, false)
}

pub fn render(fs: &mut IsoFs, out: &Path, only: &[String], scale: usize) -> Result<()> {
    for name in only {
        anyhow::ensure!(
            PARTS.contains(&name.as_str()),
            "unknown part {name}; expected one of {PARTS:?}"
        );
    }
    let want = |p: &str| only.is_empty() || only.iter().any(|o| o == p);
    let main = palette(fs, "GFX/LM3D.PAL")?;
    let o = |f: &str| out.join(f);
    if want("title-mhc") {
        let frames = title::decode_rle_cells(&read(fs, "GFX/TITLE.MHC")?, title::LOGO_WIDTH)?;
        println!(
            "TITLE.MHC: {} frames of {}x{}",
            frames.len(),
            frames[0].width,
            frames[0].height
        );
        contact(&o("title-mhc.png"), &frames, 4, scale, &main, true)?;
    }
    if want("title-menu") {
        let menu = title::MenuArt::parse(&read(fs, "GFX/TITLE.RNC")?)?;
        contact(
            &o("title-buttons.png"),
            &menu.buttons,
            5,
            scale,
            &main,
            false,
        )?;
        contact(
            &o("title-ratings.png"),
            &menu.ratings,
            5,
            scale,
            &main,
            false,
        )?;
        contact(&o("title-faces.png"), &menu.faces, 3, scale, &main, false)?;
    }
    if want("title-font") {
        font_sheet(
            &o("title-font.png"),
            &l3d_formats::font::title_font(&read(fs, "GFX/TITLE.FNT")?)?,
            scale,
            &main,
        )?;
    }
    if want("lemmings-font") {
        let l = title::LemmingLetters::parse(&read(fs, "GFX/LEMMINGS.FNT")?)?;
        // Row 0: idle frames and the closing frame; then one row per letter.
        let mut cells: Vec<IndexedImage> = l.idle().to_vec();
        cells.push(l.frames[title::LETTER_TOTAL - 1].clone());
        cells.resize(title::LETTER_FRAMES, IndexedImage::new(1, 1, vec![0])?);
        for c in b'A'..=b'Z' {
            cells.extend_from_slice(l.letter(c).unwrap_or_default());
        }
        contact(
            &o("lemmings-font.png"),
            &cells,
            title::LETTER_FRAMES,
            scale,
            &main,
            false,
        )?;
    }
    if want("icons") {
        let icons = Icons::parse(&read(fs, "GFX/ICONS.RNC")?)?;
        contact(&o("icons-small.png"), &icons.small, 16, scale, &main, false)?;
        contact(&o("icons-panel.png"), &icons.panel, 16, scale, &main, false)?;
        let mut labels = vec![icons.umbrella.clone()];
        labels.extend(icons.labels.iter().cloned());
        contact(&o("icons-labels.png"), &labels, 4, scale, &main, false)?;
        let rows = icons.unknown.len() / 12;
        let strip = IndexedImage::new(12, rows, icons.unknown[..rows * 12].to_vec())?;
        contact(
            &o("icons-unknown-w12.png"),
            &[strip],
            1,
            scale,
            &main,
            false,
        )?;
        font_sheet(&o("icons-font-small.png"), &icons.small_font, scale, &main)?;
        font_sheet(&o("icons-font-large.png"), &icons.large_font, scale, &main)?;
    }
    if want("sheets") || want("loading") {
        let loading = screen::loading(&read(fs, "GFX/LOADING.RNC")?)?;
        if want("loading") {
            contact(
                &o("loading.png"),
                std::slice::from_ref(&loading.image),
                1,
                scale,
                &loading.palette,
                false,
            )?;
        }
        if want("sheets") {
            for s in sheets::ALL {
                let pal = match s.palette {
                    Some(p) => palette(fs, p)?,
                    None => loading.palette,
                };
                let cells = s.cut(&read(fs, s.path)?)?;
                let name = s
                    .path
                    .trim_start_matches("GFX/")
                    .split('.')
                    .next()
                    .unwrap_or(s.path)
                    .to_lowercase();
                contact(
                    &o(&format!("{name}.png")),
                    &cells,
                    16.min(512 / s.cell_width),
                    scale,
                    &pal,
                    false,
                )?;
            }
        }
    }
    if want("intro") {
        for n in 1..=7 {
            let lo = screen::intro(&read(fs, &format!("GFX/INTRO{n}.RNC"))?, screen::LOW_RES)?;
            contact(
                &o(&format!("intro{n}.png")),
                &[lo.image],
                1,
                1,
                &lo.palette,
                false,
            )?;
            let hi = screen::intro(&read(fs, &format!("GFX/INTRO{n}.SVG"))?, screen::HIGH_RES)?;
            contact(
                &o(&format!("intro{n}-svg.png")),
                &[hi.image],
                1,
                1,
                &hi.palette,
                false,
            )?;
        }
    }
    if want("scene") {
        for n in 0..=10 {
            for (ext, pal_ext, size) in [
                ("RNC", "PAL", screen::LOW_RES),
                ("SVG", "SVP", screen::HIGH_RES),
            ] {
                let pal = palette(fs, &format!("GFX/SCENE{n:03}.{pal_ext}"))?;
                let s = screen::scene(&read(fs, &format!("GFX/SCENE{n:03}.{ext}"))?, size)?;
                let tag = ext.to_lowercase();
                contact(
                    &o(&format!("scene{n:03}-{tag}.png")),
                    &[s.image],
                    1,
                    1,
                    &pal,
                    false,
                )?;
                if !s.prompts.is_empty() {
                    contact(
                        &o(&format!("scene{n:03}-{tag}-prompts.png")),
                        &s.prompts,
                        4,
                        scale,
                        &pal,
                        false,
                    )?;
                }
            }
        }
    }
    if want("bmps") {
        let paths: Vec<String> = fs
            .walk()?
            .into_iter()
            .filter(|(p, e)| !e.is_dir && p.to_ascii_uppercase().starts_with("BMPS/"))
            .map(|(p, _)| p)
            .collect();
        for p in paths {
            let pic = screen::bmp(&read(fs, &p)?)?;
            println!("{p}: {}x{}", pic.image.width, pic.image.height);
            let name = p
                .trim_start_matches("BMPS/")
                .replace('/', "-")
                .to_lowercase()
                .replace(".bmp", ".png");
            contact(&o(&name), &[pic.image], 1, 1, &pic.palette, false)?;
        }
    }
    Ok(())
}

/// Names of the sprite sets accepted by `l3d-tool ui-find`.
pub const SETS: [&str; 19] = [
    "logo",
    "buttons",
    "ratings",
    "faces",
    "title-font",
    "letters",
    "icons-small",
    "panel",
    "labels",
    "font-small",
    "font-large",
    "minilemm",
    "bombnumb",
    "cogs",
    "mouse",
    "deflicon",
    "pracicon",
    "endlemms",
    "winder",
];

/// The sprite sheet whose file name, lower-cased and without extension, is
/// `name`.
fn sheet_named(name: &str) -> Option<sheets::Sheet> {
    sheets::ALL.into_iter().find(|s| {
        s.path
            .trim_start_matches("GFX/")
            .split('.')
            .next()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
    })
}

/// The cells of sprite set `name` (see [`SETS`]), in file order: for
/// `labels`, the umbrella followed by the three labels; for fonts, glyph `i`
/// is character code `33 + i`. `raw:PATH:WIDTH` is the whole file PATH as
/// one image WIDTH pixels wide.
pub fn sprite_set(fs: &mut IsoFs, name: &str) -> Result<Vec<IndexedImage>> {
    let icons =
        |fs: &mut IsoFs| -> Result<Icons> { Ok(Icons::parse(&read(fs, "GFX/ICONS.RNC")?)?) };
    let menu = |fs: &mut IsoFs| -> Result<title::MenuArt> {
        Ok(title::MenuArt::parse(&read(fs, "GFX/TITLE.RNC")?)?)
    };
    Ok(match name {
        "logo" => title::decode_rle_cells(&read(fs, "GFX/TITLE.MHC")?, title::LOGO_WIDTH)?,
        "buttons" => menu(fs)?.buttons,
        "ratings" => menu(fs)?.ratings,
        "faces" => menu(fs)?.faces,
        "title-font" => l3d_formats::font::title_font(&read(fs, "GFX/TITLE.FNT")?)?.glyphs,
        "letters" => title::LemmingLetters::parse(&read(fs, "GFX/LEMMINGS.FNT")?)?.frames,
        "icons-small" => icons(fs)?.small,
        "panel" => icons(fs)?.panel,
        "labels" => {
            let i = icons(fs)?;
            std::iter::once(i.umbrella).chain(i.labels).collect()
        }
        "font-small" => icons(fs)?.small_font.glyphs,
        "font-large" => icons(fs)?.large_font.glyphs,
        _ if name.starts_with("raw:") => {
            let (path, width) = name[4..]
                .rsplit_once(':')
                .context("expected raw:PATH:WIDTH")?;
            let width: usize = width.parse().context("raw width")?;
            anyhow::ensure!(width > 0, "raw width must be positive");
            let data = read(fs, path)?;
            let height = data.len() / width;
            vec![IndexedImage::new(
                width,
                height,
                data[..width * height].to_vec(),
            )?]
        }
        _ => match sheet_named(name) {
            Some(s) => s.cut(&read(fs, s.path)?)?,
            None => anyhow::bail!(
                "unknown sprite set {name}; expected one of {SETS:?} or raw:PATH:WIDTH"
            ),
        },
    })
}

/// One placement of a sprite cell in a searched image.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub cell: usize,
    /// Top-left corner of the cell, in image pixels.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Opaque cell pixels whose colour differs from the image.
    pub misses: usize,
    /// Opaque (non-zero) pixels in the cell.
    pub opaque: usize,
}

impl Placement {
    fn miss_ratio(&self) -> f64 {
        self.misses as f64 / self.opaque as f64
    }

    fn overlap(&self, o: &Placement) -> i32 {
        let w = (self.x + self.w).min(o.x + o.w) - self.x.max(o.x);
        let h = (self.y + self.h).min(o.y + o.h) - self.y.max(o.y);
        w.max(0) * h.max(0)
    }
}

/// Every placement of `cells` (index, image) inside `area` (X,Y,W,H of the
/// RGB8 image `img`) at which at most `max_miss` of the cell's opaque pixels
/// differ from the image by more than `tol` (summed absolute RGB
/// difference). Cells with fewer than `min_opaque` opaque pixels are
/// skipped. Placements are best first; of two that overlap by more than half
/// the smaller one, only the better is kept.
pub fn find(
    img: &(u32, u32, Vec<u8>),
    cells: &[(usize, IndexedImage)],
    pal: &Palette,
    area: [i32; 4],
    tol: i32,
    max_miss: f64,
    min_opaque: usize,
) -> Vec<Placement> {
    let (iw, ih) = (img.0 as i32, img.1 as i32);
    let [ax, ay, aw, ah] = area;
    let (x0, y0, x1, y1) = (ax.max(0), ay.max(0), (ax + aw).min(iw), (ay + ah).min(ih));
    let mut found = Vec::new();
    for (k, cell) in cells {
        let opaque: Vec<(i32, i32, [i32; 3])> = (0..cell.height)
            .flat_map(|y| (0..cell.width).map(move |x| (x, y)))
            .filter(|&(x, y)| cell.get(x, y) != 0)
            .map(|(x, y)| {
                (
                    x as i32,
                    y as i32,
                    pal[cell.get(x, y) as usize].map(i32::from),
                )
            })
            .collect();
        if opaque.len() < min_opaque.max(1) {
            continue;
        }
        let limit = (max_miss * opaque.len() as f64).floor() as usize;
        let (w, h) = (cell.width as i32, cell.height as i32);
        for y in y0..=y1 - h {
            for x in x0..=x1 - w {
                let mut misses = 0;
                for &(dx, dy, c) in &opaque {
                    let i = (((y + dy) * iw + x + dx) * 3) as usize;
                    if (0..3)
                        .map(|j| (img.2[i + j] as i32 - c[j]).abs())
                        .sum::<i32>()
                        > tol
                    {
                        misses += 1;
                        if misses > limit {
                            break;
                        }
                    }
                }
                if misses <= limit {
                    found.push(Placement {
                        cell: *k,
                        x,
                        y,
                        w,
                        h,
                        misses,
                        opaque: opaque.len(),
                    });
                }
            }
        }
    }
    found.sort_by(|a, b| {
        a.miss_ratio()
            .total_cmp(&b.miss_ratio())
            .then(b.opaque.cmp(&a.opaque))
    });
    let mut kept: Vec<Placement> = Vec::new();
    for p in found {
        if kept
            .iter()
            .all(|q| 2 * p.overlap(q) <= (p.w * p.h).min(q.w * q.h))
        {
            kept.push(p);
        }
    }
    kept
}
