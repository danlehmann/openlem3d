//! `l3d-tool ui`: renders the user-interface graphics as PNG contact sheets.

use std::path::Path;

use anyhow::{Context, Result};
use l3d_formats::image::{IndexedImage, Palette, vga_palette};
use l3d_formats::iso9660::IsoFs;
use l3d_formats::{font::Font, icons::Icons, rnc, screen, sheets, title};

/// Names accepted by `l3d-tool ui`, in rendering order.
pub const PARTS: [&str; 10] =
    ["title-mhc", "title-menu", "title-font", "lemmings-font", "icons", "sheets", "loading", "intro", "scene", "bmps"];

/// Background of contact sheets, chosen to stand apart from the game colours.
const BACKDROP: [u8; 3] = [64, 0, 64];

fn read(fs: &mut IsoFs, path: &str) -> Result<Vec<u8>> {
    Ok(rnc::unpack_if_packed(fs.read_path(path).with_context(|| format!("reading {path}"))?)?)
}

fn palette(fs: &mut IsoFs, path: &str) -> Result<Palette> {
    Ok(vga_palette(&read(fs, path)?)?)
}

/// Lays `cells` out `per_row` across, each slot as large as the largest
/// cell, `scale`× enlarged, with a 1-pixel (scaled) gap; draws index 0 as
/// the backdrop when `transparent`, and writes the result to `out`.
fn contact(out: &Path, cells: &[IndexedImage], per_row: usize, scale: usize, pal: &Palette, transparent: bool) -> Result<()> {
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
        anyhow::ensure!(PARTS.contains(&name.as_str()), "unknown part {name}; expected one of {PARTS:?}");
    }
    let want = |p: &str| only.is_empty() || only.iter().any(|o| o == p);
    let main = palette(fs, "GFX/LM3D.PAL")?;
    let o = |f: &str| out.join(f);
    if want("title-mhc") {
        let frames = title::decode_rle_cells(&read(fs, "GFX/TITLE.MHC")?, title::LOGO_WIDTH)?;
        println!("TITLE.MHC: {} frames of {}x{}", frames.len(), frames[0].width, frames[0].height);
        contact(&o("title-mhc.png"), &frames, 4, scale, &main, true)?;
    }
    if want("title-menu") {
        let menu = title::MenuArt::parse(&read(fs, "GFX/TITLE.RNC")?)?;
        contact(&o("title-buttons.png"), &menu.buttons, 5, scale, &main, false)?;
        contact(&o("title-ratings.png"), &menu.ratings, 5, scale, &main, false)?;
        contact(&o("title-faces.png"), &menu.faces, 3, scale, &main, false)?;
    }
    if want("title-font") {
        font_sheet(&o("title-font.png"), &l3d_formats::font::title_font(&read(fs, "GFX/TITLE.FNT")?)?, scale, &main)?;
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
        contact(&o("lemmings-font.png"), &cells, title::LETTER_FRAMES, scale, &main, false)?;
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
        contact(&o("icons-unknown-w12.png"), &[strip], 1, scale, &main, false)?;
        font_sheet(&o("icons-font-small.png"), &icons.small_font, scale, &main)?;
        font_sheet(&o("icons-font-large.png"), &icons.large_font, scale, &main)?;
    }
    if want("sheets") || want("loading") {
        let loading = screen::loading(&read(fs, "GFX/LOADING.RNC")?)?;
        if want("loading") {
            contact(&o("loading.png"), std::slice::from_ref(&loading.image), 1, scale, &loading.palette, false)?;
        }
        if want("sheets") {
            for s in sheets::ALL {
                let pal = match s.palette {
                    Some(p) => palette(fs, p)?,
                    None => loading.palette,
                };
                let cells = s.cut(&read(fs, s.path)?)?;
                let name = s.path.trim_start_matches("GFX/").split('.').next().unwrap_or(s.path).to_lowercase();
                contact(&o(&format!("{name}.png")), &cells, 16.min(512 / s.cell_width), scale, &pal, false)?;
            }
        }
    }
    if want("intro") {
        for n in 1..=7 {
            let lo = screen::intro(&read(fs, &format!("GFX/INTRO{n}.RNC"))?, screen::LOW_RES)?;
            contact(&o(&format!("intro{n}.png")), &[lo.image], 1, 1, &lo.palette, false)?;
            let hi = screen::intro(&read(fs, &format!("GFX/INTRO{n}.SVG"))?, screen::HIGH_RES)?;
            contact(&o(&format!("intro{n}-svg.png")), &[hi.image], 1, 1, &hi.palette, false)?;
        }
    }
    if want("scene") {
        for n in 0..=10 {
            for (ext, pal_ext, size) in [("RNC", "PAL", screen::LOW_RES), ("SVG", "SVP", screen::HIGH_RES)] {
                let pal = palette(fs, &format!("GFX/SCENE{n:03}.{pal_ext}"))?;
                let s = screen::scene(&read(fs, &format!("GFX/SCENE{n:03}.{ext}"))?, size)?;
                let tag = ext.to_lowercase();
                contact(&o(&format!("scene{n:03}-{tag}.png")), &[s.image], 1, 1, &pal, false)?;
                if !s.prompts.is_empty() {
                    contact(&o(&format!("scene{n:03}-{tag}-prompts.png")), &s.prompts, 4, scale, &pal, false)?;
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
            let name = p.trim_start_matches("BMPS/").replace('/', "-").to_lowercase().replace(".bmp", ".png");
            contact(&o(&name), &[pic.image], 1, 1, &pic.palette, false)?;
        }
    }
    Ok(())
}
