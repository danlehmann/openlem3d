//! The title screen (main menu) and the level-code screen, built from the
//! original's menu graphics read from the user's CD at run time
//! (`docs/spec/ui-graphics.md`). Layout and timings are provisional until
//! compared with the original.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::FontSize;
use l3d_formats::gamedata::Palette;
use l3d_formats::image::IndexedImage;
use l3d_formats::{screen, title};

use crate::menu::{AppState, MenuRating};
use crate::{CurrentLevel, Data};

/// Screen pixels per original 640×480 pixel are computed from the window;
/// the menu art is drawn at this multiple of its stored size at 640×480.
const ART_SCALE: f32 = 1.5;
/// Logo frames per second (provisional).
const LOGO_FPS: f32 = 15.0;
/// Lemming-letter frames per second, and the frame each letter holds at once
/// formed (both provisional).
const LETTER_FPS: f32 = 15.0;
const LETTER_HOLD_FRAME: usize = 9;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_art)
            .init_resource::<CodeText>()
            .add_systems(OnExit(AppState::Title), despawn::<TitleRoot>)
            .add_systems(Update, (spawn_title, title_input, animate_logo, show_rating).chain().run_if(in_state(AppState::Title)))
            .add_systems(OnEnter(AppState::Code), spawn_code)
            .add_systems(OnExit(AppState::Code), despawn::<TitleRoot>)
            .add_systems(Update, (code_input, animate_letters).chain().run_if(in_state(AppState::Code)));
    }
}

/// Menu graphics, uploaded once.
#[derive(Resource)]
struct Art {
    backdrop: Option<Handle<Image>>,
    logo: Vec<Handle<Image>>,
    /// Play, Code, Options, rating, Exit.
    buttons: Vec<Handle<Image>>,
    /// Centre of each button's face hole, and the face handles (5 heads × 3).
    holes: Vec<Vec2>,
    faces: Vec<Handle<Image>>,
    /// Practice, Fun, Tricky, Taxing, Mayhem.
    ratings: Vec<Handle<Image>>,
    letters: Option<title::LemmingLetters>,
    letter_images: Vec<Handle<Image>>,
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

/// The centre of the face hole of a menu button: index-0 pixels not
/// connected to the image border.
fn hole_centre(img: &IndexedImage) -> Vec2 {
    let (w, h) = (img.width, img.height);
    let mut outside = vec![false; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let i = y * w + x;
        if outside[i] || img.pixels[i] != 0 {
            continue;
        }
        outside[i] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let (mut sum, mut n) = (Vec2::ZERO, 0.0);
    for y in 0..h {
        for x in 0..w {
            if img.pixels[y * w + x] == 0 && !outside[y * w + x] {
                sum += Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                n += 1.0;
            }
        }
    }
    if n > 0.0 { sum / n } else { Vec2::new(w as f32 / 2.0, h as f32 / 3.0) }
}

fn load_art(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else { return };
    let mut add = |img: &IndexedImage, p: &Palette, t: bool| images.add(to_image(img, p, t));
    let backdrop = (|| {
        let pic = screen::scene(&d.read("GFX/SCENE000.SVG").ok()?, screen::HIGH_RES).ok()?;
        let p = d.palette("GFX/SCENE000.SVP").ok()?;
        Some(add(&pic.image, &p, false))
    })();
    let logo = d
        .read("GFX/TITLE.MHC")
        .ok()
        .and_then(|raw| title::decode_rle_cells(&raw, title::LOGO_WIDTH).ok())
        .map(|frames| frames.iter().map(|f| add(f, &pal, true)).collect())
        .unwrap_or_default();
    let menu = d.read("GFX/TITLE.RNC").ok().and_then(|raw| title::MenuArt::parse(&raw).ok());
    let (buttons, holes, faces, ratings) = match &menu {
        Some(m) => (
            m.buttons.iter().map(|b| add(b, &pal, true)).collect(),
            m.buttons.iter().map(hole_centre).collect(),
            m.faces.iter().map(|f| add(f, &pal, true)).collect(),
            m.ratings.iter().map(|r| add(r, &pal, true)).collect(),
        ),
        None => Default::default(),
    };
    let letters = d.read("GFX/LEMMINGS.FNT").ok().and_then(|raw| title::LemmingLetters::parse(&raw).ok());
    let letter_images = letters.as_ref().map(|l| l.frames.iter().map(|f| add(f, &pal, true)).collect()).unwrap_or_default();
    commands.insert_resource(Art { backdrop, logo, buttons, holes, faces, ratings, letters, letter_images });
}

/// Root of the title and code screens.
#[derive(Component)]
struct TitleRoot;

#[derive(Component)]
struct Logo;

/// Which rating sign image to show: the rating index into `Art::ratings`.
#[derive(Component)]
struct RatingSign;

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

fn despawn<T: Component>(mut commands: Commands, roots: Query<Entity, With<T>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn root_node(art: &Art) -> impl Bundle {
    (
        TitleRoot,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceEvenly,
            ..default()
        },
        BackgroundColor(Color::BLACK),
        art.backdrop.clone().map(|h| ImageNode::new(h).with_mode(NodeImageMode::Stretch)).unwrap_or_default(),
    )
}

/// Builds the title screen when it is shown and doesn't exist yet (the art is
/// loaded at startup, which may finish after the first state is entered).
fn spawn_title(mut commands: Commands, art: Option<Res<Art>>, roots: Query<(), With<TitleRoot>>) {
    let Some(art) = art else { return };
    if !roots.is_empty() {
        return;
    }
    let s = ART_SCALE;
    commands.spawn(root_node(&art)).with_children(|root| {
        if let Some(first) = art.logo.first() {
            root.spawn((Logo, ImageNode::new(first.clone()), Node { width: px(256.0 * s * 1.5), height: px(64.0 * s * 1.5), ..default() }));
        }
        root.spawn(Node { column_gap: px(16.0 * s), ..default() }).with_children(|row| {
            for (i, kind) in TITLE_BUTTONS.iter().enumerate() {
                let Some(img) = art.buttons.get(i) else { continue };
                row.spawn((
                    Button,
                    *kind,
                    ImageNode::new(img.clone()),
                    Node { width: px(64.0 * s), height: px(64.0 * s), ..default() },
                ))
                .with_children(|b| {
                    // The face, centred in the button's hole.
                    if let (Some(face), Some(c)) = (art.faces.get(i * 3), art.holes.get(i)) {
                        b.spawn((
                            ImageNode::new(face.clone()),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px((c.x - 8.0) * s),
                                top: px((c.y - 8.0) * s),
                                width: px(16.0 * s),
                                height: px(16.0 * s),
                                ..default()
                            },
                        ));
                    }
                    if *kind == TitleButton::Rating {
                        b.spawn((
                            RatingSign,
                            ImageNode::default(),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(16.0 * s),
                                top: px(36.0 * s),
                                width: px(32.0 * s),
                                height: px(32.0 * s),
                                ..default()
                            },
                        ));
                    }
                });
            }
        });
    });
}

/// `menu::RATINGS` order (Fun, Tricky, Taxing, Mayhem, Practice) to the
/// rating-sign order (Practice, Fun, Tricky, Taxing, Mayhem).
fn sign_index(rating: usize) -> usize {
    (rating + 1) % 5
}

fn show_rating(art: Option<Res<Art>>, rating: Res<MenuRating>, mut signs: Query<&mut ImageNode, With<RatingSign>>) {
    let Some(art) = art else { return };
    for mut node in &mut signs {
        if let Some(h) = art.ratings.get(sign_index(rating.0)) {
            node.image = h.clone();
        }
    }
}

fn animate_logo(art: Option<Res<Art>>, time: Res<Time>, mut logos: Query<&mut ImageNode, With<Logo>>) {
    let Some(art) = art else { return };
    if art.logo.is_empty() {
        return;
    }
    let frame = (time.elapsed_secs() * LOGO_FPS) as usize % art.logo.len();
    for mut node in &mut logos {
        node.image = art.logo[frame].clone();
    }
}

fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &TitleButton), Changed<Interaction>>,
    mut rating: ResMut<MenuRating>,
    mut next: ResMut<NextState<AppState>>,
    mut muted: ResMut<crate::music::MusicMuted>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut pressed: Vec<TitleButton> =
        buttons.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, b)| *b).collect();
    let keymap = [
        (KeyCode::F1, TitleButton::Play),
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
        match b {
            TitleButton::Play => next.set(AppState::Menu),
            TitleButton::Code => next.set(AppState::Code),
            // Options are not designed yet; toggling music is a stand-in.
            TitleButton::Options => muted.0 = !muted.0,
            TitleButton::Rating => rating.0 = (rating.0 + 1) % 5,
            TitleButton::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// The code typed so far, with the time each letter was typed.
#[derive(Resource, Default)]
struct CodeText {
    letters: Vec<(u8, f32)>,
    message: String,
}

#[derive(Component)]
struct LetterRow;

#[derive(Component)]
struct CodeMessage;

fn spawn_code(mut commands: Commands, art: Option<Res<Art>>, mut code: ResMut<CodeText>) {
    let Some(art) = art else { return };
    code.letters.clear();
    code.message = "Type a level code, Enter to start, Esc to go back".into();
    commands.spawn(root_node(&art)).with_children(|root| {
        root.spawn((LetterRow, Node { column_gap: px(4.0), min_height: px(32.0 * ART_SCALE * 2.0), ..default() }));
        root.spawn((CodeMessage, Text::new(""), TextFont { font_size: FontSize::Px(20.0), ..default() }));
    });
}

const MAX_CODE_LEN: usize = 8;

fn code_input(
    mut events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    time: Res<Time>,
    mut code: ResMut<CodeText>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    use bevy::input::keyboard::Key;
    let now = time.elapsed_secs();
    for k in events.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Character(s) => {
                for c in s.chars().filter(|c| c.is_ascii_alphabetic()) {
                    if code.letters.len() < MAX_CODE_LEN {
                        code.letters.push((c.to_ascii_uppercase() as u8, now));
                    }
                }
            }
            Key::Backspace => {
                code.letters.pop();
            }
            Key::Escape => next.set(AppState::Title),
            Key::Enter => {
                let text: String = code.letters.iter().map(|(c, _)| *c as char).collect();
                match crate::codes::level_for_code(&text) {
                    Some(n) => {
                        current.number = n;
                        current.loaded = None;
                        next.set(AppState::Playing);
                    }
                    None => {
                        code.message = format!("{text} is not a known code");
                        code.letters.clear();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Rebuilds the row of lemming letters: each typed letter plays its
/// forming animation and then holds.
fn animate_letters(
    mut commands: Commands,
    art: Option<Res<Art>>,
    time: Res<Time>,
    code: Res<CodeText>,
    rows: Query<(Entity, Option<&Children>), With<LetterRow>>,
    mut messages: Query<&mut Text, With<CodeMessage>>,
) {
    let Some(art) = art else { return };
    let Some(letters) = &art.letters else { return };
    let now = time.elapsed_secs();
    for (row, children) in &rows {
        if let Some(children) = children {
            for c in children.iter() {
                commands.entity(c).despawn();
            }
        }
        let size = 32.0 * ART_SCALE * 2.0;
        for &(c, typed) in &code.letters {
            let Some(frames) = letters.letter(c) else { continue };
            let step = ((now - typed) * LETTER_FPS) as usize;
            let frame_in_letter = step.min(LETTER_HOLD_FRAME).min(frames.len() - 1);
            let index = title::LETTER_IDLE_FRAMES + (c - b'A') as usize * title::LETTER_FRAMES + frame_in_letter;
            if let Some(h) = art.letter_images.get(index) {
                let child = commands.spawn((ImageNode::new(h.clone()), Node { width: px(size), height: px(size), ..default() })).id();
                commands.entity(row).add_child(child);
            }
        }
    }
    for mut t in &mut messages {
        if t.0 != code.message {
            t.0 = code.message.clone();
        }
    }
}
