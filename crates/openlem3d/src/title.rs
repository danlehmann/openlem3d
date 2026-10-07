//! The title screen (main menu) and the level-code screen, laid out like the
//! original's (`docs/spec/ui-graphics.md`, "Screens"): its graphics, read
//! from the user's CD, placed at the measured 320×200 positions on a canvas
//! scaled to fit the window. The scrolling backdrop fills the whole window.
//!
//! Nothing here makes the player wait: every screen takes input from its
//! first frame, and the animations never block (`docs/GROUNDRULES.md`).

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use l3d_formats::font::{Font, title_font};
use l3d_formats::gamedata::Palette;
use l3d_formats::icons::Icons;
use l3d_formats::image::IndexedImage;
use l3d_formats::{sheets, title};

use crate::menu::{AppState, MenuRating};
use crate::{CurrentLevel, Data};

/// The original's screen, in which all positions are given.
const SCREEN: Vec2 = Vec2::new(320.0, 200.0);
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
const PAPER: Rect = Rect { min: Vec2::new(63.0, 93.0), max: Vec2::new(257.0, 128.0) };
const BANNER_LINES: [f32; 2] = [97.0, 111.0];
/// Lemming letters and the idle slots: images per second (70 Hz ÷ 6).
const LETTER_FPS: f32 = 70.0 / 6.0;
/// How long "Password Correct" shows before the level (any key skips it).
const CORRECT_SHOWN: f32 = 0.6;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_art)
            .init_resource::<CodeText>()
            .add_systems(OnExit(AppState::Title), despawn::<ScreenRoot>)
            .add_systems(Update, attract.run_if(in_state(AppState::Title)))
            .add_systems(
                Update,
                (spawn_title, title_input, animate_logo, animate_winders, animate_banner, animate_faces, show_rating)
                    .chain()
                    .run_if(in_state(AppState::Title)),
            )
            .add_systems(OnExit(AppState::Code), despawn::<ScreenRoot>)
            .add_systems(Update, (spawn_code, code_input, animate_slots).chain().run_if(in_state(AppState::Code)))
            .add_systems(
                Update,
                scroll_backdrop.run_if(
                    in_state(AppState::Title)
                        .or_else(in_state(AppState::Code))
                        .or_else(in_state(AppState::Options))
                        .or_else(in_state(AppState::Practice))
                        .or_else(in_state(AppState::Menu)),
                ),
            )
            .add_systems(OnExit(AppState::Options), despawn::<ScreenRoot>)
            .add_systems(PostUpdate, layout.before(bevy::ui::UiSystems::Layout));
    }
}

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
            match c.checked_sub(self.first).map(|i| i as usize).filter(|&i| i < self.images.len()) {
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
#[derive(Resource)]
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

fn to_image(img: &IndexedImage, pal: &Palette, transparent0: bool) -> Image {
    let mut image = Image::new(
        Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.to_rgba(pal, transparent0),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// Advance of a proportional glyph: one less than its last opaque column,
/// counted from 1 (measured for the large font on the code screen).
fn advance(g: &IndexedImage) -> f32 {
    let last = (0..g.width).rev().find(|&x| (0..g.height).any(|y| g.pixels[y * g.width + x] != 0));
    last.map_or(0.0, |x| x as f32)
}

/// `gap` is added to each glyph's advance: 0 for the italic fonts, whose
/// glyphs overlap by a pixel, 1 for the upright small font.
pub(crate) fn glyphs(f: &Font, pal: &Palette, space: f32, gap: f32, images: &mut Assets<Image>) -> Glyphs {
    Glyphs {
        first: f.first,
        images: f.glyphs.iter().map(|g| images.add(to_image(g, pal, true))).collect(),
        // Picture tiles (codes 123 and up in TITLE.FNT) join edge to edge.
        advances: f.glyphs.iter().enumerate().map(|(i, g)| if f.first as usize + i >= 123 { g.width as f32 } else { advance(g) + gap }).collect(),
        size: Vec2::new(f.glyph_width as f32, f.glyph_height as f32),
        space,
    }
}

fn load_art(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else { return };
    let mut add = |img: &IndexedImage, t: bool| images.add(to_image(img, &pal, t));
    let backdrop = d.gfx("BGRD", 0).ok().and_then(|raw| IndexedImage::new(320, raw.len() / 320, raw).ok()).map(|i| add(&i, false));
    let logo = d
        .read("GFX/TITLE.MHC")
        .ok()
        .and_then(|raw| title::decode_rle_cells(&raw, title::LOGO_WIDTH).ok())
        .map(|frames| frames.iter().map(|f| add(f, true)).collect())
        .unwrap_or_default();
    let menu = d.read("GFX/TITLE.RNC").ok().and_then(|raw| title::MenuArt::parse(&raw).ok());
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
    let rating_labels = ["PRACTICE", "FUN", "TRICKY", "TAXING", "MAYHEM"]
        .iter()
        .map(|name| {
            let img = rating_label(name);
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
    let banner = d.read("GFX/TITLE.FNT").ok().and_then(|raw| title_font(&raw).ok()).map(|f| glyphs(&f, &pal, 5.0, 0.0, &mut images));
    let large = d.read("GFX/ICONS.RNC").ok().and_then(|raw| Icons::parse(&raw).ok()).map(|i| glyphs(&i.large_font, &pal, 5.0, 0.0, &mut images));
    commands.insert_resource(Art { backdrop, logo, buttons, faces, ratings, rating_labels, winder, banner, large, letters });
}

/// The rating label's box is at least wide enough to cover x 213–231.
const LABEL_MIN_WIDTH: usize = 19;

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
/// 0x40–0x50), on a black box one pixel larger on every side (measured),
/// widened to hide the button's own "FUN" label (5-pixel letters at x
/// 214–230) under short names.
fn rating_label(name: &str) -> IndexedImage {
    let n = name.chars().count();
    let (w, h) = ((4 * n + 1).max(LABEL_MIN_WIDTH), 7);
    let pad = (w - (4 * n + 1)) / 2;
    let mut pixels = vec![0u8; w * h];
    // Index 0 is black and drawn opaque here.
    for (i, c) in name.chars().enumerate() {
        for (y, row) in letter_rows(c).iter().enumerate() {
            for (x, b) in row.bytes().enumerate() {
                if b == b'#' {
                    pixels[(y + 1) * w + pad + 1 + 4 * i + x] = 0x40 + 4 * y as u8;
                }
            }
        }
    }
    IndexedImage::new(w, h, pixels).expect("label size")
}

/// Root of the title and code screens.
#[derive(Component)]
pub(crate) struct ScreenRoot;

/// A canvas of the given size in the original's pixels (320×200 for the
/// title and code screens), scaled to fit the window and centred. One
/// exists at a time.
#[derive(Component)]
pub(crate) struct Canvas(pub(crate) Vec2);

/// The backdrop's tiles, and the scroll speed of the current screen.
#[derive(Component)]
struct Backdrop(f32);

/// An element's rectangle in canvas pixels, relative to its parent.
#[derive(Component, Clone, Copy)]
pub(crate) struct At(pub(crate) Rect);

pub(crate) fn at(x: f32, y: f32, w: f32, h: f32) -> At {
    At(Rect::new(x, y, x + w, y + h))
}

#[derive(Component)]
struct Logo;

#[derive(Component)]
struct Winder {
    mirrored: bool,
}

/// The strip of banner pages that slides across the paper.
#[derive(Component)]
struct BannerStrip {
    pages: usize,
}

#[derive(Component)]
struct Face(usize);

#[derive(Component)]
struct RatingSign;

#[derive(Component)]
struct RatingLabel;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum TitleButton {
    Play,
    Code,
    Options,
    Rating,
    Exit,
}

const TITLE_BUTTONS: [TitleButton; 5] =
    [TitleButton::Play, TitleButton::Code, TitleButton::Options, TitleButton::Rating, TitleButton::Exit];

/// Face positions inside their buttons (measured).
const FACE_OFFSETS: [Vec2; 5] =
    [Vec2::new(26.0, 5.0), Vec2::new(21.0, 10.0), Vec2::new(17.0, 6.0), Vec2::new(25.0, 4.0), Vec2::new(24.0, 6.0)];
const BUTTON_Y: f32 = 135.0;

pub(crate) fn despawn<T: Component>(mut commands: Commands, roots: Query<Entity, With<T>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

pub(crate) fn image_at(image: Handle<Image>, rect: At) -> (ImageNode, At, Node) {
    (ImageNode::new(image), rect, Node { position_type: PositionType::Absolute, ..default() })
}

/// Spawns the root, the backdrop (tinted by `tint`) and the canvas; returns
/// the canvas.
pub(crate) fn spawn_screen(commands: &mut Commands, art: &Art, scroll: f32, tint: Color) -> Entity {
    let root = commands
        .spawn((
            ScreenRoot,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), overflow: Overflow::clip(), ..default() },
            BackgroundColor(Color::BLACK),
        ))
        .id();
    let backdrop = commands.spawn((Backdrop(scroll), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(root).add_child(backdrop);
    if let Some(bg) = &art.backdrop {
        // Enough tiles to cover any window at the canvas scale.
        for _ in 0..(3 * 8) {
            let tile = commands.spawn((ImageNode::new(bg.clone()).with_color(tint), Node { position_type: PositionType::Absolute, ..default() })).id();
            commands.entity(backdrop).add_child(tile);
        }
    }
    let canvas = commands.spawn((Canvas(SCREEN), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(root).add_child(canvas);
    canvas
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
fn spawn_title(mut commands: Commands, art: Option<Res<Art>>, roots: Query<(), With<ScreenRoot>>) {
    let Some(art) = art else { return };
    if !roots.is_empty() {
        return;
    }
    let canvas = spawn_screen(&mut commands, &art, TITLE_SCROLL, Color::WHITE);
    let child = |commands: &mut Commands, b: &mut dyn FnMut(&mut Commands) -> Entity| {
        let e = b(commands);
        commands.entity(canvas).add_child(e);
    };
    if let Some(first) = art.logo.first() {
        child(&mut commands, &mut |c| c.spawn((Logo, image_at(first.clone(), at(32.0, 10.0, 256.0, 64.0)))).id());
    }
    // The banner: paper, then a clipped strip of pages, then the winders.
    let paper = at(PAPER.min.x, PAPER.min.y, PAPER.width(), PAPER.height());
    child(&mut commands, &mut |c| c.spawn((paper, Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::WHITE))).id());
    if let Some(font) = &art.banner {
        let pages = banner_pages();
        let window = commands
            .spawn((at(PAPER.min.x, 0.0, PAPER.width(), SCREEN.y), Node { position_type: PositionType::Absolute, overflow: Overflow::clip(), ..default() }))
            .id();
        commands.entity(canvas).add_child(window);
        let strip = commands.spawn((BannerStrip { pages: pages.len() }, at(0.0, 0.0, 1.0, 1.0), Node { position_type: PositionType::Absolute, ..default() })).id();
        commands.entity(window).add_child(strip);
        let page_w = PAPER.width();
        for (p, lines) in pages.iter().enumerate() {
            for (line, y) in lines.iter().zip(BANNER_LINES) {
                let (glyphs, width) = font.layout(line);
                // Page p is centred at strip x (p + 1.5)·page width.
                let left = (p as f32 + 1.5) * page_w - width / 2.0;
                for (x, image) in glyphs {
                    let g = commands.spawn(image_at(image, at(left + x, y, font.size.x, font.size.y))).id();
                    commands.entity(strip).add_child(g);
                }
            }
        }
    }
    if let Some(cell) = art.winder.first() {
        child(&mut commands, &mut |c| c.spawn((Winder { mirrored: false }, image_at(cell.clone(), at(0.0, 70.0, 64.0, 64.0)))).id());
        child(&mut commands, &mut |c| {
            let mut img = ImageNode::new(cell.clone());
            img.flip_x = true;
            c.spawn((Winder { mirrored: true }, img, at(256.0, 70.0, 64.0, 64.0), Node { position_type: PositionType::Absolute, ..default() })).id()
        });
    }
    // Faces under the buttons, then the buttons, the sign and its label.
    for (k, offset) in FACE_OFFSETS.iter().enumerate() {
        if let Some(face) = art.faces.get(3 * k) {
            let r = at(64.0 * k as f32 + offset.x, BUTTON_Y + offset.y, 16.0, 16.0);
            child(&mut commands, &mut |c| c.spawn((Face(k), image_at(face.clone(), r))).id());
        }
    }
    for (k, kind) in TITLE_BUTTONS.iter().enumerate() {
        if let Some(img) = art.buttons.get(k) {
            let r = at(64.0 * k as f32, BUTTON_Y, 64.0, 64.0);
            child(&mut commands, &mut |c| c.spawn((Button, *kind, image_at(img.clone(), r))).id());
        }
    }
    child(&mut commands, &mut |c| c.spawn((RatingSign, image_at(Handle::default(), at(207.0, 153.0, 32.0, 32.0)), Pickable::IGNORE)).id());
    child(&mut commands, &mut |c| c.spawn((RatingLabel, image_at(Handle::default(), at(0.0, 0.0, 0.0, 0.0)), Pickable::IGNORE)).id());
}

/// Fits the canvas into the window and places every element on it; tiles
/// the backdrop across the whole window.
fn layout(
    windows: Query<&Window>,
    mut canvases: Query<(&Canvas, &mut Node), Without<At>>,
    mut items: Query<(&At, &mut Node), Without<Canvas>>,
) {
    let Ok(window) = windows.single() else { return };
    let Some((size, _)) = canvases.iter().next().map(|(c, _)| (c.0, ())) else { return };
    let (w, h) = (window.width(), window.height());
    let s = (w / size.x).min(h / size.y);
    for (_, mut node) in &mut canvases {
        node.left = px((w - size.x * s) / 2.0);
        node.top = px((h - size.y * s) / 2.0);
        node.width = px(size.x * s);
        node.height = px(size.y * s);
    }
    for (a, mut node) in &mut items {
        node.left = px(a.0.min.x * s);
        node.top = px(a.0.min.y * s);
        node.width = px(a.0.width() * s);
        node.height = px(a.0.height() * s);
    }
}

/// The backdrop scrolls upwards and wraps every 48 rows; columns line up
/// with the canvas.
fn scroll_backdrop(
    time: Res<Time>,
    windows: Query<&Window>,
    backdrops: Query<(&Backdrop, &Children)>,
    mut tiles: Query<&mut Node, Without<Backdrop>>,
) {
    const TILE: Vec2 = Vec2::new(320.0, 48.0);
    let Ok(window) = windows.single() else { return };
    let (w, h) = (window.width(), window.height());
    let s = (w / SCREEN.x).min(h / SCREEN.y);
    let origin = Vec2::new((w - SCREEN.x * s) / 2.0, (h - SCREEN.y * s) / 2.0);
    let size = TILE * s;
    for (b, children) in &backdrops {
        let phase = (time.elapsed_secs() * b.0).rem_euclid(TILE.y) * s;
        let x0 = origin.x.rem_euclid(size.x) - size.x;
        let y0 = (origin.y - phase).rem_euclid(size.y) - size.y;
        let cols = ((w - x0) / size.x).ceil().max(1.0) as usize;
        for (i, child) in children.iter().enumerate() {
            let Ok(mut node) = tiles.get_mut(child) else { continue };
            let (cx, cy) = (i % cols, i / cols);
            let (x, y) = (x0 + cx as f32 * size.x, y0 + cy as f32 * size.y);
            node.left = px(x);
            node.top = px(y);
            node.width = px(size.x);
            node.height = px(size.y);
            node.display = if cx < cols && y < h { Display::Flex } else { Display::None };
        }
    }
}

/// `menu::RATINGS` order (Fun, Tricky, Taxing, Mayhem, Practice) to the
/// rating-sign order (Practice, Fun, Tricky, Taxing, Mayhem).
fn sign_index(rating: usize) -> usize {
    (rating + 1) % 5
}

fn show_rating(
    art: Option<Res<Art>>,
    rating: Res<MenuRating>,
    mut signs: Query<&mut ImageNode, (With<RatingSign>, Without<RatingLabel>)>,
    mut labels: Query<(&mut ImageNode, &mut At), With<RatingLabel>>,
) {
    let Some(art) = art else { return };
    let i = sign_index(rating.0);
    for mut node in &mut signs {
        if let Some(h) = art.ratings.get(i)
            && node.image != *h
        {
            node.image = h.clone();
        }
    }
    for (mut node, mut a) in &mut labels {
        if let Some((h, size)) = art.rating_labels.get(i) {
            if node.image != *h {
                node.image = h.clone();
            }
            // Text rows y 179–183; "PRACTICE" spans x 208–238 (measured).
            let left = if size.x as usize > LABEL_MIN_WIDTH { 223.0 - (size.x / 2.0).floor() } else { 213.0 };
            *a = at(left, 178.0, size.x, size.y);
        }
    }
}

/// Frames 0–48 at 35 frames/s, then frame 0 held for 1.4 s.
fn animate_logo(art: Option<Res<Art>>, time: Res<Time>, mut logos: Query<&mut ImageNode, With<Logo>>) {
    let Some(art) = art else { return };
    if art.logo.is_empty() {
        return;
    }
    let turn = art.logo.len() as f32 / LOGO_FPS;
    let t = time.elapsed_secs() % (turn + LOGO_HOLD);
    let frame = if t < turn { (t * LOGO_FPS) as usize % art.logo.len() } else { 0 };
    for mut node in &mut logos {
        if node.image != art.logo[frame] {
            node.image = art.logo[frame].clone();
        }
    }
}

/// Banner timing: how far the strip has moved, and whether a page is
/// being held centred.
fn banner_offset(t: f32) -> (f32, bool) {
    let travel = PAPER.width() / BANNER_SPEED;
    let period = travel + BANNER_HOLD;
    let k = (t / period).floor();
    let u = t - k * period;
    (k * PAPER.width() + BANNER_SPEED * u.min(travel), u >= travel)
}

/// The time since the title screen was entered (the banner restarts then).
#[derive(Component)]
struct Since(f32);

fn animate_banner(
    mut commands: Commands,
    time: Res<Time>,
    mut strips: Query<(Entity, &BannerStrip, &mut At, Option<&Since>)>,
) {
    for (e, strip, mut a, since) in &mut strips {
        let Some(since) = since else {
            commands.entity(e).insert(Since(time.elapsed_secs()));
            continue;
        };
        let t = time.elapsed_secs() - since.0;
        // The pages repeat once all have passed.
        let total = strip.pages as f32 * (PAPER.width() / BANNER_SPEED + BANNER_HOLD);
        let (offset, _) = banner_offset(t % total);
        *a = at(-offset, 0.0, 1.0, 1.0);
    }
}

/// The winders turn their rollers while the banner moves and rest while it
/// holds a page: the left one plays cells 7…0, the right one (mirrored)
/// 0…7.
fn animate_winders(
    art: Option<Res<Art>>,
    time: Res<Time>,
    strips: Query<&Since, With<BannerStrip>>,
    mut winders: Query<(&Winder, &mut ImageNode)>,
    mut last: Local<(f32, usize)>,
) {
    let Some(art) = art else { return };
    if art.winder.is_empty() {
        return;
    }
    let held = strips.iter().next().is_some_and(|s| banner_offset(time.elapsed_secs() - s.0).1);
    if !held {
        last.0 += time.delta_secs() * WINDER_FPS;
        while last.0 >= 1.0 {
            last.0 -= 1.0;
            last.1 = (last.1 + 1) % art.winder.len();
        }
    }
    let n = art.winder.len();
    for (w, mut node) in &mut winders {
        let cell = if w.mirrored { last.1 } else { (n - 1) - last.1 };
        if node.image != art.winder[cell] {
            node.image = art.winder[cell].clone();
        }
    }
}

/// One face at a time blinks: open, half, closed, half, open; blinks start
/// 0.6–1.1 s apart, the face chosen at random (estimated).
fn animate_faces(
    art: Option<Res<Art>>,
    time: Res<Time>,
    mut faces: Query<(&Face, &mut ImageNode)>,
    mut blink: Local<(f32, usize, u32)>,
) {
    /// (cell offset, duration) after the blink starts.
    const STEPS: [(usize, f32); 3] = [(1, 0.07), (2, 0.05), (1, 0.04)];
    let Some(art) = art else { return };
    let t = time.elapsed_secs();
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
    for (f, mut node) in &mut faces {
        let c = if f.0 == blink.1 { cell } else { 0 };
        if let Some(h) = art.faces.get(3 * f.0 + c)
            && node.image != *h
        {
            node.image = h.clone();
        }
    }
}
fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &TitleButton), Changed<Interaction>>,
    mut rating: ResMut<MenuRating>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<crate::sfx::Sfx>,
) {
    let mut pressed: Vec<TitleButton> =
        buttons.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, b)| *b).collect();
    let keymap = [
        (KeyCode::F1, TitleButton::Play),
        (KeyCode::Enter, TitleButton::Play),
        (KeyCode::F2, TitleButton::Code),
        (KeyCode::F12, TitleButton::Options),
        (KeyCode::Escape, TitleButton::Exit),
    ];
    pressed.extend(keymap.iter().filter(|(k, _)| keys.just_pressed(*k)).map(|(_, b)| *b));
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
            sfx.write(crate::sfx::Sfx(v));
        }
        match b {
            // Practice has its own menu.
            TitleButton::Play => next.set(if rating.0 == crate::menu::PRACTICE { AppState::Practice } else { AppState::Menu }),
            TitleButton::Code => next.set(AppState::Code),
            TitleButton::Options => next.set(AppState::Options),
            TitleButton::Rating => rating.0 = (rating.0 + 1) % 5,
            TitleButton::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// The code screen's state.
#[derive(Resource, Default)]
struct CodeText {
    /// Typed letters with the time each was typed.
    letters: Vec<(u8, f32)>,
    /// Letters being rubbed out, by slot, with the time.
    erasing: Vec<(usize, u8, f32)>,
    /// When a valid code was accepted, and its level.
    accepted: Option<(f32, u32)>,
}

#[derive(Component)]
struct Slot(usize);

#[derive(Component, Clone)]
struct CorrectText;

const MAX_CODE_LEN: usize = 8;

/// A line of large-font text centred on x = 160 at height `y`.
fn centred_line(commands: &mut Commands, parent: Entity, font: &Glyphs, text: &str, y: f32, marker: impl Bundle + Clone) {
    let (glyphs, width) = font.layout(text);
    let left = (160.0 - width / 2.0).round();
    for (x, image) in glyphs {
        let g = commands.spawn((image_at(image, at(left + x, y, font.size.x, font.size.y)), marker.clone())).id();
        commands.entity(parent).add_child(g);
    }
}

#[derive(Component, Clone)]
struct Plain;

/// Builds the code screen when it is shown and doesn't exist yet.
fn spawn_code(mut commands: Commands, art: Option<Res<Art>>, roots: Query<(), With<ScreenRoot>>, mut code: ResMut<CodeText>) {
    let Some(art) = art else { return };
    if !roots.is_empty() {
        return;
    }
    *code = CodeText::default();
    let canvas = spawn_screen(&mut commands, &art, CODE_SCROLL, Color::WHITE);
    if let Some(font) = &art.large {
        centred_line(&mut commands, canvas, font, "Enter Password", 60.0, Plain);
        centred_line(&mut commands, canvas, font, "Password Correct", 140.0, (CorrectText, Visibility::Hidden));
        centred_line(&mut commands, canvas, font, "Press Return when finished", 160.0, Plain);
    }
    for k in 0..MAX_CODE_LEN {
        let s = commands.spawn((Slot(k), image_at(Handle::default(), at(32.0 + 34.0 * k as f32, 90.0, 32.0, 32.0)))).id();
        commands.entity(canvas).add_child(s);
    }
}

fn code_input(
    mut events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut code: ResMut<CodeText>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<crate::sfx::Sfx>,
) {
    use bevy::input::keyboard::Key;
    let now = time.elapsed_secs();
    let mut any = mouse.get_just_pressed().next().is_some();
    for k in events.read() {
        if !k.state.is_pressed() {
            continue;
        }
        any = true;
        if code.accepted.is_some() {
            continue;
        }
        match &k.logical_key {
            Key::Character(s) => {
                for c in s.chars().filter(|c| c.is_ascii_alphabetic()) {
                    if code.letters.len() < MAX_CODE_LEN {
                        code.letters.push((c.to_ascii_uppercase() as u8, now));
                        sfx.write(crate::sfx::Sfx("VOXFX/HUP3"));
                    }
                }
            }
            Key::Backspace => {
                if let Some((c, _)) = code.letters.pop() {
                    let slot = code.letters.len();
                    code.erasing.retain(|e| e.0 != slot);
                    code.erasing.push((slot, c, now));
                }
            }
            Key::Escape => next.set(AppState::Title),
            Key::Enter => {
                let text: String = code.letters.iter().map(|(c, _)| *c as char).collect();
                match crate::codes::level_for_code(&text) {
                    Some(n) => {
                        code.accepted = Some((now, n));
                        sfx.write(crate::sfx::Sfx("SPOTFX/POSITIVE"));
                    }
                    None => {
                        // An unknown code: the letters drop back into rows
                        // (the original returns to the title instead).
                        sfx.write(crate::sfx::Sfx("SPOTFX/NEGATIVE"));
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
        next.set(AppState::Briefing);
    }
}

/// Slot images: idle slots loop the six row frames and the plain row, each
/// slot one image ahead of the one to its left (the last two in step); typed
/// letters play frames 0–15 then loop 12–15; rubbed-out letters play
/// 16–19.
fn animate_slots(
    art: Option<Res<Art>>,
    time: Res<Time>,
    code: Res<CodeText>,
    mut slots: Query<(&Slot, &mut ImageNode)>,
    mut correct: Query<&mut Visibility, With<CorrectText>>,
) {
    let Some(art) = art else { return };
    if art.letters.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let step = (now * LETTER_FPS) as usize;
    let letter = |c: u8, f: usize| title::LETTER_IDLE_FRAMES + (c - b'A') as usize * title::LETTER_FRAMES + f;
    for (slot, mut node) in &mut slots {
        let k = slot.0;
        let index = if let Some(&(c, typed)) = code.letters.get(k) {
            let f = ((now - typed) * LETTER_FPS) as usize;
            letter(c, if f < 16 { f } else { 12 + (f - 16) % 4 })
        } else if let Some(&(_, c, when)) = code.erasing.iter().find(|e| e.0 == k && ((now - e.2) * LETTER_FPS) < 4.0) {
            letter(c, 16 + ((now - when) * LETTER_FPS) as usize)
        } else {
            (step + k.min(MAX_CODE_LEN - 2)) % (title::LETTER_IDLE_FRAMES + 1)
        };
        if let Some(h) = art.letters.get(index)
            && node.image != *h
        {
            node.image = h.clone();
        }
    }
    for mut v in &mut correct {
        *v = if code.accepted.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// Seconds on the title screen before a demo plays, as the original's
/// attract mode (about 125 s, observed). Unlike the original, any input
/// restarts the wait.
const ATTRACT_AFTER: f32 = 125.0;

/// Attract mode: after a while without input, the next Practice demo plays;
/// any input, or its end, returns to the title.
#[allow(clippy::too_many_arguments)]
fn attract(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    touches: Res<Touches>,
    mut data: ResMut<Data>,
    mut game: ResMut<crate::Game>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
    mut idle: Local<f32>,
    mut turn: Local<usize>,
) {
    let input = keys.get_pressed().next().is_some() || mouse.get_pressed().next().is_some() || motion.delta != Vec2::ZERO || touches.iter().next().is_some();
    *idle = if input { 0.0 } else { *idle + time.delta_secs() };
    if *idle < ATTRACT_AFTER {
        return;
    }
    *idle = 0.0;
    let demos: Vec<u32> = l3d_sim::demos::SOLUTIONS.iter().map(|s| s.level).filter(|&n| n >= crate::menu::PRACTICE as u32 * crate::menu::LEVELS_PER_RATING).collect();
    let n = demos[*turn % demos.len()];
    *turn += 1;
    let Some(log) = data.0.level(n).ok().zip(data.0.blocks(n).ok()).and_then(|(level, blocks)| l3d_sim::demos::demo(n, &level, &blocks)) else { return };
    current.number = n;
    current.loaded = None;
    game.replay = Some(crate::Replay::demo(log, AppState::Title));
    next.set(AppState::Playing);
}
