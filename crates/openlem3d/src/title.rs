//! The title screen (main menu) and the level-code screen, laid out like the
//! original's (`docs/spec/ui-graphics.md`, "Screens"): its graphics, read
//! from the user's CD, placed at the measured 320×200 positions on a canvas
//! scaled to fit the window. The scrolling backdrop fills the whole window.
//!
//! Nothing here makes the player wait: every screen takes input from its
//! first frame, and the animations never block (`docs/GROUNDRULES.md`).

use l3d_engine::input::KeyboardInput;
use l3d_engine::prelude::*;
use l3d_formats::font::{Font, title_font};
use l3d_formats::gamedata::Palette;
use l3d_formats::icons::Icons;
use l3d_formats::image::IndexedImage;
use l3d_formats::{sheets, title};

use crate::World;
use crate::menu::AppState;
use crate::texture::indexed_image;
use crate::ui::{Node, NodeId, Ui, UiNode, percent};

/// The original's screen, in which all positions are given.
const SCREEN: Vec2 = Vec2::new(320.0, 200.0);
/// The backdrop tile, `BGRD.000`.
const TILE: Vec2 = Vec2::new(320.0, 48.0);
/// The backdrop as drawn: the tile widened by `BACKDROP_SIDE` columns on
/// each side and repeated down `BACKDROP_ROWS` times, enough to cover windows
/// from 5:1 to 1:2.4.
const BACKDROP_SIDE: usize = 360;
const BACKDROP_ROWS: usize = 17;
/// Backdrop scroll speeds (pixels per second; estimated).
const TITLE_SCROLL: f32 = 35.0;
const CODE_SCROLL: f32 = 23.0;
/// Logo: 35 frames per second, then the face-on frame held for 1.4 s.
const LOGO_FPS: f32 = 35.0;
const LOGO_HOLD: f32 = 1.4;
/// Winders: cells per second.
const WINDER_FPS: f32 = 17.5;
/// Banner: pages move left at this speed and hold this long when centred
/// (observed: about 70 px/s for about 3.1 s, then about 0.7 s still).
const BANNER_SPEED: f32 = 70.0;
const BANNER_HOLD: f32 = 0.7;
/// The banner paper (x 63–256, y 93–127) and its text lines.
const PAPER: Rect = Rect {
    min: Vec2::new(63.0, 93.0),
    max: Vec2::new(257.0, 128.0),
};
const BANNER_LINES: [f32; 2] = [97.0, 111.0];
/// Lemming letters and the idle slots: images per second (70 Hz ÷ 6).
const LETTER_FPS: f32 = 70.0 / 6.0;
/// How long "Password Correct" shows before the level (any key skips it).
const CORRECT_SHOWN: f32 = 0.6;

/// A font as uploaded images plus each glyph's advance.
pub(crate) struct Glyphs {
    first: u8,
    images: Vec<Handle<Image>>,
    advances: Vec<f32>,
    pub(crate) size: Vec2,
    space: f32,
}

impl Glyphs {
    /// Glyph images with their x offsets for `text`, and the total width.
    pub(crate) fn layout(&self, text: &str) -> (Vec<(f32, Handle<Image>)>, f32) {
        let (mut x, mut out) = (0.0, Vec::new());
        // Characters are glyph codes (picture tiles go up to 198).
        for c in text.chars().map(|c| u8::try_from(c as u32).unwrap_or(b' ')) {
            match c
                .checked_sub(self.first)
                .map(|i| i as usize)
                .filter(|&i| i < self.images.len())
            {
                Some(i) if c != b' ' => {
                    out.push((x, self.images[i].clone()));
                    x += self.advances[i];
                }
                _ => x += self.space,
            }
        }
        (out, x)
    }
}

/// Menu graphics, uploaded once.
/// The menus' graphics.
pub(crate) struct Art {
    backdrop: Option<Handle<Image>>,
    logo: Vec<Handle<Image>>,
    /// Play, Code, Options, rating, Exit.
    buttons: Vec<Handle<Image>>,
    /// Five heads × (open, half, closed).
    faces: Vec<Handle<Image>>,
    /// Practice, Fun, Tricky, Taxing, Mayhem.
    ratings: Vec<Handle<Image>>,
    /// The rating names in our own 3×5 lettering, same order.
    rating_labels: Vec<(Handle<Image>, Vec2)>,
    winder: Vec<Handle<Image>>,
    banner: Option<Glyphs>,
    pub(crate) large: Option<Glyphs>,
    letters: Vec<Handle<Image>>,
}

/// Advance of a proportional glyph: one less than its last opaque column,
/// counted from 1 (measured for the large font on the code screen).
/// Advance of a proportional glyph: one less than its last opaque column,
/// counted from 1 (measured for the large font on the code screen).
fn advance(g: &IndexedImage) -> f32 {
    let last = (0..g.width)
        .rev()
        .find(|&x| (0..g.height).any(|y| g.pixels[y * g.width + x] != 0));
    last.map_or(0.0, |x| x as f32)
}

/// `gap` is added to each glyph's advance: 0 for the italic fonts, whose
/// glyphs overlap by a pixel, 1 for the upright small font.
pub(crate) fn glyphs(
    f: &Font,
    pal: &Palette,
    space: f32,
    gap: f32,
    images: &mut Assets<Image>,
) -> Glyphs {
    Glyphs {
        first: f.first,
        images: f
            .glyphs
            .iter()
            .map(|g| images.add(indexed_image(g, pal, true)))
            .collect(),
        // Picture tiles (codes 123 and up in TITLE.FNT) join edge to edge.
        advances: f
            .glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| {
                if f.first as usize + i >= 123 {
                    g.width as f32
                } else {
                    advance(g) + gap
                }
            })
            .collect(),
        size: Vec2::new(f.glyph_width as f32, f.glyph_height as f32),
        space,
    }
}

/// `BGRD.000` (`raw`) as one picture to cover any window, so that no seams
/// show. The tile does not join to itself across: its logos are 120 pixels
/// apart. But columns 0–57 and 173–199 equal the columns 120 to their right
/// (verified), so beside the tile the pattern continues exactly by repeating
/// columns 200–319 to the right and 0–119 to the left.
fn widened_backdrop(raw: &[u8]) -> Option<IndexedImage> {
    const PERIOD: i32 = 120;
    let (w, h) = (TILE.x as i32, TILE.y as usize);
    if raw.len() != w as usize * h {
        return None;
    }
    let width = w as usize + 2 * BACKDROP_SIDE;
    let column = |x: i32| match x {
        x if x < 0 => x.rem_euclid(PERIOD),
        x if x >= w => w - PERIOD + (x - w) % PERIOD,
        x => x,
    } as usize;
    let mut pixels = Vec::with_capacity(width * h * BACKDROP_ROWS);
    for y in 0..h * BACKDROP_ROWS {
        let row = &raw[(y % h) * w as usize..][..w as usize];
        pixels.extend((0..width).map(|x| row[column(x as i32 - BACKDROP_SIDE as i32)]));
    }
    IndexedImage::new(width, h * BACKDROP_ROWS, pixels).ok()
}

pub(crate) fn load_art(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else {
        return;
    };
    let mut add = |img: &IndexedImage, t: bool| images.add(indexed_image(img, &pal, t));
    let backdrop = d
        .gfx("BGRD", 0)
        .ok()
        .and_then(|raw| widened_backdrop(&raw))
        .map(|i| add(&i, false));
    let logo = d
        .read("GFX/TITLE.MHC")
        .ok()
        .and_then(|raw| title::decode_rle_cells(&raw, title::LOGO_WIDTH).ok())
        .map(|frames| frames.iter().map(|f| add(f, true)).collect())
        .unwrap_or_default();
    let menu = d
        .read("GFX/TITLE.RNC")
        .ok()
        .and_then(|raw| title::MenuArt::parse(&raw).ok());
    let (buttons, faces, ratings) = match &menu {
        Some(m) => (
            m.buttons.iter().map(|b| add(b, true)).collect(),
            m.faces.iter().map(|f| add(f, true)).collect(),
            m.ratings.iter().map(|r| add(r, true)).collect(),
        ),
        None => Default::default(),
    };
    let winder = d
        .read(sheets::WINDER.path)
        .ok()
        .and_then(|raw| sheets::WINDER.cut(&raw).ok())
        .map(|cells| cells.iter().map(|c| add(c, true)).collect())
        .unwrap_or_default();
    // The card's white: the palette entry nearest pure white.
    let white = (1..=255u8)
        .max_by_key(|&i| pal[i as usize].iter().map(|&c| c as u32).sum::<u32>())
        .unwrap_or(15);
    let rating_labels = ["PRACTICE", "FUN", "TRICKY", "TAXING", "MAYHEM"]
        .iter()
        .map(|name| {
            let img = rating_label(name, white);
            let size = Vec2::new(img.width as f32, img.height as f32);
            (add(&img, false), size)
        })
        .collect();
    let letters = d
        .read("GFX/LEMMINGS.FNT")
        .ok()
        .and_then(|raw| title::LemmingLetters::parse(&raw).ok())
        .map(|l| l.frames.iter().map(|f| add(f, true)).collect())
        .unwrap_or_default();
    let banner = d
        .read("GFX/TITLE.FNT")
        .ok()
        .and_then(|raw| title_font(&raw).ok())
        .map(|f| glyphs(&f, &pal, 5.0, 0.0, images));
    let large = d
        .read("GFX/ICONS.RNC")
        .ok()
        .and_then(|raw| Icons::parse(&raw).ok())
        .map(|i| glyphs(&i.large_font, &pal, 5.0, 0.0, images));
    w.art = Some(Art {
        backdrop,
        logo,
        buttons,
        faces,
        ratings,
        rating_labels,
        winder,
        banner,
        large,
        letters,
    });
}

/// The rating label's box: every label is drawn this wide, on the card's
/// white with its name centred, so it covers the label printed in the card's own
/// box (x 207–239) whatever the rating.
const LABEL_WIDTH: usize = 33;

/// Our own 3×5 capitals for the rating label (the original's lettering is
/// drawn by the program, not stored in the data files). One string per
/// letter, rows top to bottom.
fn letter_rows(c: char) -> [&'static str; 5] {
    match c {
        'A' => ["###", "#.#", "###", "#.#", "#.#"],
        'C' => ["###", "#..", "#..", "#..", "###"],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'F' => ["###", "#..", "##.", "#..", "#.."],
        'G' => ["###", "#..", "#.#", "#.#", "###"],
        'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'K' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'P' => ["###", "#.#", "###", "#..", "#.."],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        'Y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

/// A rating name as the original shows it under the sign: 3×5 capitals with
/// a 1-pixel gap, each row coloured from the red-to-yellow ramp (indices
/// 0x40–0x50), each letter outlined one pixel wide in black, straight on the
/// white card (seen in the original). The label is drawn over a box of the
/// card's white (`white`), [`LABEL_WIDTH`] wide, hiding the button's own
/// printed label.
fn rating_label(name: &str, white: u8) -> IndexedImage {
    let n = name.chars().count();
    let (w, h) = ((4 * n + 1).max(LABEL_WIDTH), 7);
    let pad = (w - (4 * n + 1)) / 2;
    let mut pixels = vec![white; w * h];
    let mut ink = vec![false; w * h];
    for (i, c) in name.chars().enumerate() {
        for (y, row) in letter_rows(c).iter().enumerate() {
            for (x, b) in row.bytes().enumerate() {
                if b == b'#' {
                    let p = (y + 1) * w + pad + 1 + 4 * i + x;
                    pixels[p] = 0x40 + 4 * y as u8;
                    ink[p] = true;
                }
            }
        }
    }
    // The outline: black (index 0, drawn opaque here) around every letter
    // pixel, diagonals included.
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = (y as usize) * w + x as usize;
            let near = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..w as i32).contains(&nx)
                        && (0..h as i32).contains(&ny)
                        && ink[ny as usize * w + nx as usize]
                })
            });
            if !ink[p] && near {
                pixels[p] = 0;
            }
        }
    }
    IndexedImage::new(w, h, pixels).expect("label size")
}

/// A canvas screen (the title, code, options, practice and level-list
/// screens; one at a time): its root, its canvas and its scrolling backdrop
/// with the backdrop's speed.
pub(crate) struct ScreenUi {
    pub(crate) root: NodeId,
    pub(crate) canvas: NodeId,
    backdrop: Option<(NodeId, f32)>,
}

/// Takes the canvas screen away, if there is one.
pub(crate) fn close_screen(ui: &mut Ui, screen: &mut Option<ScreenUi>) {
    if let Some(s) = screen.take() {
        ui.remove(s.root);
    }
}

/// Builds the root, the backdrop (tinted by `tint`) and the canvas.
pub(crate) fn spawn_screen(ui: &mut Ui, art: &Art, scroll: f32, tint: Color) -> ScreenUi {
    let root = ui.root(
        UiNode::new()
            .layout(Node {
                width: percent(100),
                height: percent(100),
                clip: true,
                ..default()
            })
            .background(Color::BLACK),
    );
    let backdrop = art
        .backdrop
        .as_ref()
        .map(|bg| (ui.child(root, UiNode::image(bg.clone()).tint(tint)), scroll));
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    ScreenUi {
        root,
        canvas,
        backdrop,
    }
}

/// The title screen's and the code screen's moving parts, and what their
/// animations keep between frames.
#[derive(Default)]
pub(crate) struct Title {
    /// The title's canvas, while it is up.
    pub(crate) canvas: Option<NodeId>,
    logo: Option<NodeId>,
    /// The banner's strip of pages, how many, and when it started.
    strip: Option<(NodeId, usize, f32)>,
    winders: Vec<(NodeId, bool)>,
    faces: Vec<(NodeId, usize)>,
    buttons: Vec<(NodeId, TitleButton)>,
    sign: Option<NodeId>,
    label: Option<NodeId>,
    winder: (f32, usize),
    blink: (f32, usize, u32),
    idle: f32,
    turn: usize,
    code: CodeText,
    slots: Vec<NodeId>,
    correct: Vec<NodeId>,
    area: Option<NodeId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleButton {
    Play,
    Code,
    Options,
    Rating,
    Exit,
}

const TITLE_BUTTONS: [TitleButton; 5] = [
    TitleButton::Play,
    TitleButton::Code,
    TitleButton::Options,
    TitleButton::Rating,
    TitleButton::Exit,
];

/// Face positions inside their buttons (measured).
const FACE_OFFSETS: [Vec2; 5] = [
    Vec2::new(26.0, 5.0),
    Vec2::new(21.0, 10.0),
    Vec2::new(17.0, 6.0),
    Vec2::new(25.0, 4.0),
    Vec2::new(24.0, 6.0),
];
const BUTTON_Y: f32 = 135.0;

/// Leaving a canvas screen takes it away.
pub(crate) fn exit_screen(w: &mut World) {
    close_screen(&mut w.ui, &mut w.screen_ui);
    w.title.canvas = None;
}

/// The banner's pages: lines of `TITLE.FNT` text. Codes 123–148 are the two
/// rows of the "Psygnosis" wordmark and 149–160 and 165–176 the two rows of the "3D Lemmings" logo, as
/// the original's banner starts. The credits that follow are our own.
fn banner_pages() -> Vec<[String; 2]> {
    let tiles = |r: std::ops::RangeInclusive<u8>| r.map(|c| c as char).collect::<String>();
    vec![
        [tiles(123..=135), tiles(136..=148)],
        ["Proudly Presents.".into(), String::new()],
        [tiles(149..=160), tiles(165..=176)],
        ["openlem3d".into(), "a clean-room remake".into()],
        ["Original game by".into(), "Clockwork Games".into()],
        ["Published by".into(), "Psygnosis, 1995".into()],
        ["Play with the data".into(), "from your own CD".into()],
    ]
}

/// Builds the title screen when it is shown and doesn't exist yet (the art is
/// loaded at startup, which may finish after the first state is entered).
fn spawn_title(w: &mut World) {
    let Some(art) = &w.art else { return };
    if w.screen_ui.is_some() {
        return;
    }
    let now = w.e.time.game.elapsed_secs();
    let ui = &mut w.ui;
    let screen = spawn_screen(ui, art, TITLE_SCROLL, Color::WHITE);
    let canvas = screen.canvas;
    w.screen_ui = Some(screen);
    let t = &mut w.title;
    t.canvas = Some(canvas);
    t.winders.clear();
    t.faces.clear();
    t.buttons.clear();
    t.logo = art.logo.first().map(|first| {
        ui.child(
            canvas,
            UiNode::image(first.clone()).at(32.0, 10.0, 256.0, 64.0),
        )
    });
    // The banner: paper, then a clipped strip of pages, then the winders.
    ui.child(
        canvas,
        UiNode::new()
            .at(PAPER.min.x, PAPER.min.y, PAPER.width(), PAPER.height())
            .background(Color::WHITE),
    );
    t.strip = None;
    if let Some(font) = &art.banner {
        let pages = banner_pages();
        let window = ui.child(
            canvas,
            UiNode::new()
                .at(PAPER.min.x, 0.0, PAPER.width(), SCREEN.y)
                .layout(Node {
                    clip: true,
                    ..default()
                }),
        );
        let strip = ui.child(window, UiNode::new().at(0.0, 0.0, 1.0, 1.0));
        t.strip = Some((strip, pages.len(), now));
        let page_w = PAPER.width();
        for (p, lines) in pages.iter().enumerate() {
            for (line, y) in lines.iter().zip(BANNER_LINES) {
                let (glyphs, width) = font.layout(line);
                // Page p is centred at strip x (p + 1.5)·page width.
                let left = (p as f32 + 1.5) * page_w - width / 2.0;
                for (x, image) in glyphs {
                    ui.child(
                        strip,
                        UiNode::image(image).at(left + x, y, font.size.x, font.size.y),
                    );
                }
            }
        }
    }
    if let Some(cell) = art.winder.first() {
        let left = ui.child(
            canvas,
            UiNode::image(cell.clone()).at(0.0, 70.0, 64.0, 64.0),
        );
        let right = ui.child(
            canvas,
            UiNode::image(cell.clone())
                .flip_x(true)
                .at(256.0, 70.0, 64.0, 64.0),
        );
        t.winders = vec![(left, false), (right, true)];
    }
    // Faces under the buttons, then the buttons, the sign and its label.
    for (k, offset) in FACE_OFFSETS.iter().enumerate() {
        if let Some(face) = art.faces.get(3 * k) {
            let id = ui.child(
                canvas,
                UiNode::image(face.clone()).at(
                    64.0 * k as f32 + offset.x,
                    BUTTON_Y + offset.y,
                    16.0,
                    16.0,
                ),
            );
            t.faces.push((id, k));
        }
    }
    for (k, kind) in TITLE_BUTTONS.iter().enumerate() {
        if let Some(img) = art.buttons.get(k) {
            let id = ui.child(
                canvas,
                UiNode::image(img.clone())
                    .button()
                    .at(64.0 * k as f32, BUTTON_Y, 64.0, 64.0),
            );
            t.buttons.push((id, *kind));
        }
    }
    t.sign = Some(ui.child(
        canvas,
        UiNode::image(Handle::default()).at(207.0, 153.0, 32.0, 32.0),
    ));
    t.label = Some(ui.child(
        canvas,
        UiNode::image(Handle::default()).at(0.0, 0.0, 0.0, 0.0),
    ));
}

/// The backdrop scrolls upwards and wraps every 48 rows, its columns lined up
/// with the canvas.
pub(crate) fn scroll_backdrop(w: &mut World) {
    let Some((node, speed)) = w.screen_ui.as_ref().and_then(|s| s.backdrop) else {
        return;
    };
    let (width, height) = (w.screen.size.x, w.screen.size.y);
    let s = (width / SCREEN.x).min(height / SCREEN.y);
    let origin = Vec2::new((width - SCREEN.x * s) / 2.0, (height - SCREEN.y * s) / 2.0);
    let phase = (w.e.time.game.elapsed_secs() * speed).rem_euclid(TILE.y) * s;
    if let Some(n) = w.ui.get_mut(node) {
        n.node.left = crate::ui::px(origin.x - BACKDROP_SIDE as f32 * s);
        n.node.top = crate::ui::px((origin.y - phase).rem_euclid(TILE.y * s) - TILE.y * s);
        n.node.width = crate::ui::px((TILE.x + 2.0 * BACKDROP_SIDE as f32) * s);
        n.node.height = crate::ui::px(TILE.y * BACKDROP_ROWS as f32 * s);
    }
}

/// `menu::RATINGS` order (Fun, Tricky, Taxing, Mayhem, Practice) to the
/// rating-sign order (Practice, Fun, Tricky, Taxing, Mayhem).
fn sign_index(rating: usize) -> usize {
    (rating + 1) % 5
}

fn show_rating(w: &mut World) {
    let Some(art) = &w.art else { return };
    let i = sign_index(w.rating.0);
    let t = &w.title;
    if let (Some(h), Some(sign)) = (art.ratings.get(i), t.sign) {
        w.ui.set_image(sign, h);
    }
    if let (Some((h, size)), Some(label)) = (art.rating_labels.get(i), t.label)
        && let Some(n) = w.ui.get_mut(label)
    {
        n.set_image(h);
        // Text rows y 179–183; "PRACTICE" spans x 208–238 (measured).
        let left = 223.0 - (size.x / 2.0).floor();
        n.at = Some(Rect::new(left, 178.0, left + size.x, 178.0 + size.y));
    }
}

fn animate_logo(w: &mut World) {
    let Some(art) = &w.art else { return };
    if art.logo.is_empty() {
        return;
    }
    let turn = art.logo.len() as f32 / LOGO_FPS;
    let t = w.e.time.game.elapsed_secs() % (turn + LOGO_HOLD);
    let frame = if t < turn {
        (t * LOGO_FPS) as usize % art.logo.len()
    } else {
        0
    };
    if let Some(logo) = w.title.logo {
        w.ui.set_image(logo, &art.logo[frame]);
    }
}

/// Banner timing: how far the strip has moved, and whether a page is
/// being held centred.
fn banner_offset(t: f32) -> (f32, bool) {
    let travel = PAPER.width() / BANNER_SPEED;
    let period = travel + BANNER_HOLD;
    let k = (t / period).floor();
    let u = t - k * period;
    (
        k * PAPER.width() + BANNER_SPEED * u.min(travel),
        u >= travel,
    )
}

/// The time since the title screen was entered (the banner restarts then).
fn animate_banner(w: &mut World) {
    let Some((strip, pages, since)) = w.title.strip else {
        return;
    };
    let t = w.e.time.game.elapsed_secs() - since;
    // The pages repeat once all have passed.
    let total = pages as f32 * (PAPER.width() / BANNER_SPEED + BANNER_HOLD);
    let (offset, _) = banner_offset(t % total);
    if let Some(n) = w.ui.get_mut(strip) {
        n.at = Some(Rect::new(-offset, 0.0, -offset + 1.0, 1.0));
    }
}

fn animate_winders(w: &mut World) {
    let Some(art) = &w.art else { return };
    if art.winder.is_empty() {
        return;
    }
    let time = w.e.time.game;
    let t = &mut w.title;
    let held = t
        .strip
        .is_some_and(|(_, _, since)| banner_offset(time.elapsed_secs() - since).1);
    let last = &mut t.winder;
    if !held {
        last.0 += time.delta_secs() * WINDER_FPS;
        while last.0 >= 1.0 {
            last.0 -= 1.0;
            last.1 = (last.1 + 1) % art.winder.len();
        }
    }
    let n = art.winder.len();
    for &(id, mirrored) in &t.winders {
        let cell = if mirrored { last.1 } else { (n - 1) - last.1 };
        w.ui.set_image(id, &art.winder[cell]);
    }
}

fn animate_faces(w: &mut World) {
    const STEPS: [(usize, f32); 3] = [(1, 0.07), (2, 0.05), (1, 0.04)];
    let Some(art) = &w.art else { return };
    let t = w.e.time.game.elapsed_secs();
    let blink = &mut w.title.blink;
    let length: f32 = STEPS.iter().map(|s| s.1).sum();
    if t >= blink.0 + length {
        // A small linear congruential generator picks the next face and gap.
        let mut next = || {
            blink.2 = blink.2.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (blink.2 >> 16) & 0x7FFF
        };
        let face = next() as usize % FACE_OFFSETS.len();
        let gap = 0.6 + (next() % 500) as f32 / 1000.0;
        *blink = (blink.0.max(t - length) + length + gap, face, blink.2);
    }
    let mut elapsed = t - blink.0;
    let mut cell = 0;
    if elapsed >= 0.0 {
        for (c, d) in STEPS {
            if elapsed < d {
                cell = c;
                break;
            }
            elapsed -= d;
        }
    }
    let blinking = blink.1;
    for &(id, k) in &w.title.faces {
        let c = if k == blinking { cell } else { 0 };
        if let Some(h) = art.faces.get(3 * k + c) {
            w.ui.set_image(id, h);
        }
    }
}

fn title_input(w: &mut World) {
    let keys = &w.e.input.keys;
    let mut pressed: Vec<TitleButton> = w
        .title
        .buttons
        .iter()
        .filter(|&&(id, _)| w.ui.just_pressed(id))
        .map(|&(_, b)| b)
        .collect();
    let keymap = [
        (KeyCode::F1, TitleButton::Play),
        (KeyCode::Enter, TitleButton::Play),
        (KeyCode::F2, TitleButton::Code),
        (KeyCode::F12, TitleButton::Options),
        (KeyCode::Escape, TitleButton::Exit),
    ];
    pressed.extend(
        keymap
            .iter()
            .filter(|(k, _)| keys.just_pressed(*k))
            .map(|(_, b)| *b),
    );
    let rating = &mut w.rating;
    if keys.just_pressed(KeyCode::ArrowUp) {
        rating.0 = (rating.0 + 1) % 5;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        rating.0 = (rating.0 + 4) % 5;
    }
    for b in pressed {
        let voice = match b {
            TitleButton::Play => Some("VOXFX/PLAY"),
            TitleButton::Code => Some("VOXFX/CODE"),
            TitleButton::Options => Some("VOXFX/OPTIONS"),
            TitleButton::Exit => Some("VOXFX/BYEBYE1"),
            TitleButton::Rating => None,
        };
        if let Some(v) = voice {
            w.sfx.play(v);
        }
        match b {
            // Practice has its own menu.
            TitleButton::Play => {
                w.next_state = Some(if rating.0 == crate::menu::PRACTICE {
                    AppState::Practice
                } else {
                    AppState::Menu
                })
            }
            TitleButton::Code => w.next_state = Some(AppState::Code),
            TitleButton::Options => w.next_state = Some(AppState::Options),
            TitleButton::Rating => rating.0 = (rating.0 + 1) % 5,
            TitleButton::Exit => w.e.exit(),
        }
    }
}

/// The title screen's systems, in order.
pub(crate) fn update_title(w: &mut World) {
    spawn_title(w);
    title_input(w);
    animate_logo(w);
    animate_winders(w);
    animate_banner(w);
    animate_faces(w);
    show_rating(w);
}

/// Brings up or puts away the on-screen keyboard for typing a code, where
/// the platform has one (`platform::ON_SCREEN_KEYBOARD`).
pub(crate) fn keyboard(w: &mut World, shown: bool) {
    if crate::platform::ON_SCREEN_KEYBOARD {
        w.e.window.ime_enabled = shown;
    }
}

/// Tapping the code area brings the on-screen keyboard back.
fn keyboard_again(w: &mut World) {
    if crate::platform::ON_SCREEN_KEYBOARD && w.title.area.is_some_and(|a| w.ui.just_pressed(a)) {
        crate::platform::show_keyboard();
    }
}

/// The code screen's state.
#[derive(Default)]
struct CodeText {
    /// Typed letters with the time each was typed.
    letters: Vec<(u8, f32)>,
    /// Letters being rubbed out, by slot, with the time.
    erasing: Vec<(usize, u8, f32)>,
    /// When a valid code was accepted, and its level.
    accepted: Option<(f32, u32)>,
}

const MAX_CODE_LEN: usize = 8;

/// A line of large-font text centred on x = 160 at height `y`.
/// A line of text centred on the canvas; returns its glyphs.
fn centred_line(
    ui: &mut Ui,
    parent: NodeId,
    font: &Glyphs,
    text: &str,
    y: f32,
    shown: bool,
) -> Vec<NodeId> {
    let (glyphs, width) = font.layout(text);
    let left = (160.0 - width / 2.0).round();
    glyphs
        .into_iter()
        .map(|(x, image)| {
            ui.child(
                parent,
                UiNode::image(image)
                    .at(left + x, y, font.size.x, font.size.y)
                    .visible_if(shown),
            )
        })
        .collect()
}

/// Builds the code screen when it is shown and doesn't exist yet.
fn spawn_code(w: &mut World) {
    let Some(art) = &w.art else { return };
    if w.screen_ui.is_some() {
        return;
    }
    let t = &mut w.title;
    t.code = CodeText::default();
    let ui = &mut w.ui;
    let screen = spawn_screen(ui, art, CODE_SCROLL, Color::WHITE);
    let canvas = screen.canvas;
    w.screen_ui = Some(screen);
    t.correct.clear();
    if let Some(font) = &art.large {
        centred_line(ui, canvas, font, "Enter Password", 60.0, true);
        t.correct = centred_line(ui, canvas, font, "Password Correct", 140.0, false);
        // Tapping the prompt is Return, for players without a keyboard, as
        // "Back" is Esc.
        let prompt = "Press Return when finished";
        let (_, width) = font.layout(prompt);
        crate::key_buttons::spawn_text_button(
            ui,
            &mut w.key_buttons,
            canvas,
            font,
            prompt,
            (160.0 - width / 2.0).round(),
            160.0,
            Color::WHITE,
            KeyCode::Enter,
        );
        crate::key_buttons::spawn_text_button(
            ui,
            &mut w.key_buttons,
            canvas,
            font,
            "Back",
            4.0,
            4.0,
            Color::WHITE,
            KeyCode::Escape,
        );
    }
    // Tapping the code brings the on-screen keyboard back, if put away.
    t.area = Some(
        ui.child(
            canvas,
            UiNode::new()
                .button()
                .at(32.0, 90.0, 34.0 * MAX_CODE_LEN as f32, 32.0),
        ),
    );
    t.slots = (0..MAX_CODE_LEN)
        .map(|k| {
            ui.child(
                canvas,
                UiNode::image(Handle::default()).at(32.0 + 34.0 * k as f32, 90.0, 32.0, 32.0),
            )
        })
        .collect();
}

fn code_input(w: &mut World) {
    let events: Vec<KeyboardInput> = w.e.input.keyboard.clone();
    let mouse = &w.e.input.mouse;
    let time = w.e.time.game;
    let code = &mut w.title.code;
    let (current, next, sfx) = (&mut w.current, &mut w.next_state, &mut w.sfx);
    use l3d_engine::input::{Key, NamedKey};
    let now = time.elapsed_secs();
    let mut any = mouse.get_just_pressed().next().is_some();
    for k in &events {
        if !k.state.is_pressed() {
            continue;
        }
        any = true;
        if code.accepted.is_some() {
            continue;
        }
        // Return, Backspace and Esc by the key too: Android reports Return
        // (from its on-screen keyboard at least) as the character "\n".
        let key = match k.key_code {
            Some(KeyCode::Enter | KeyCode::NumpadEnter) => Key::Named(NamedKey::Enter),
            Some(KeyCode::Backspace) => Key::Named(NamedKey::Backspace),
            Some(KeyCode::Escape) => Key::Named(NamedKey::Escape),
            _ => k.logical_key.clone(),
        };
        match &key {
            Key::Character(s) => {
                for c in s.chars().filter(|c| c.is_ascii_alphabetic()) {
                    if code.letters.len() < MAX_CODE_LEN {
                        code.letters.push((c.to_ascii_uppercase() as u8, now));
                        sfx.play("VOXFX/HUP3");
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some((c, _)) = code.letters.pop() {
                    let slot = code.letters.len();
                    code.erasing.retain(|e| e.0 != slot);
                    code.erasing.push((slot, c, now));
                }
            }
            Key::Named(NamedKey::Escape) => *next = Some(AppState::Title),
            Key::Named(NamedKey::Enter) => {
                let text: String = code.letters.iter().map(|(c, _)| *c as char).collect();
                match crate::codes::level_for_code(&text) {
                    Some(n) => {
                        code.accepted = Some((now, n));
                        sfx.play("SPOTFX/POSITIVE");
                    }
                    None => {
                        // An unknown code: the letters drop back into rows
                        // (the original returns to the title instead).
                        sfx.play("SPOTFX/NEGATIVE");
                        let letters = std::mem::take(&mut code.letters);
                        for (slot, (c, _)) in letters.into_iter().enumerate() {
                            code.erasing.push((slot, c, now));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some((at, n)) = code.accepted
        && (now - at >= CORRECT_SHOWN || (any && now > at))
    {
        current.number = n;
        current.loaded = None;
        *next = Some(AppState::Briefing);
    }
}

/// Slot images: idle slots loop the six row frames and the plain row, each
/// slot one image ahead of the one to its left (the last two in step); typed
/// letters play frames 0–15 then loop 12–15; rubbed-out letters play
/// 16–19.
fn animate_slots(w: &mut World) {
    let (art, time, code) = (w.art.as_ref(), w.e.time.game, &w.title.code);
    let Some(art) = art else { return };
    if art.letters.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let step = (now * LETTER_FPS) as usize;
    let letter = |c: u8, f: usize| {
        title::LETTER_IDLE_FRAMES + (c - b'A') as usize * title::LETTER_FRAMES + f
    };
    let mut images = Vec::new();
    for (k, &id) in w.title.slots.iter().enumerate() {
        let index = if let Some(&(c, typed)) = code.letters.get(k) {
            let f = ((now - typed) * LETTER_FPS) as usize;
            letter(c, if f < 16 { f } else { 12 + (f - 16) % 4 })
        } else if let Some(&(_, c, when)) = code
            .erasing
            .iter()
            .find(|e| e.0 == k && ((now - e.2) * LETTER_FPS) < 4.0)
        {
            letter(c, 16 + ((now - when) * LETTER_FPS) as usize)
        } else {
            (step + k.min(MAX_CODE_LEN - 2)) % (title::LETTER_IDLE_FRAMES + 1)
        };
        if let Some(h) = art.letters.get(index) {
            images.push((id, h.clone()));
        }
    }
    let accepted = code.accepted.is_some();
    for (id, h) in images {
        w.ui.set_image(id, &h);
    }
    for &id in &w.title.correct {
        w.ui.set_visible(id, accepted);
    }
}

/// Seconds on the title screen before a demo plays, as the original's
/// attract mode (observed twice: about 125 s and about 118 s). Unlike the
/// original, any input restarts the wait.
const ATTRACT_AFTER: f32 = 120.0;

/// Attract mode: after a while without input, the next Practice demo plays;
/// any input, or its end, returns to the title.
pub(crate) fn attract(w: &mut World) {
    let (keys, mouse, motion, touches) = (
        &w.e.input.keys,
        &w.e.input.mouse,
        &w.e.input.motion,
        &w.e.input.touches,
    );
    let time = w.e.time.game;
    let (data, game, current, next) = (&mut w.data, &mut w.game, &mut w.current, &mut w.next_state);
    let title = &mut w.title;
    let input = keys.get_pressed().next().is_some()
        || mouse.get_pressed().next().is_some()
        || motion.delta != Vec2::ZERO
        || touches.iter().next().is_some();
    title.idle = if input {
        0.0
    } else {
        title.idle + time.delta_secs()
    };
    if title.idle < ATTRACT_AFTER {
        return;
    }
    title.idle = 0.0;
    let demos: Vec<u32> = l3d_sim::demos::SOLUTIONS
        .iter()
        .map(|s| s.level)
        .filter(|&n| n >= crate::menu::PRACTICE as u32 * crate::menu::LEVELS_PER_RATING)
        .collect();
    let n = demos[title.turn % demos.len()];
    title.turn += 1;
    let Some(log) = data
        .0
        .level(n)
        .ok()
        .zip(data.0.blocks(n).ok())
        .and_then(|(level, blocks)| l3d_sim::demos::demo(n, &level, &blocks))
    else {
        return;
    };
    current.number = n;
    current.loaded = None;
    game.replay = Some(crate::Replay::demo(log, AppState::Title));
    *next = Some(AppState::Playing);
}

/// The code screen's systems, in order.
pub(crate) fn update_code(w: &mut World) {
    spawn_code(w);
    code_input(w);
    animate_slots(w);
    keyboard_again(w);
}
