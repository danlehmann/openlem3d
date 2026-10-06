//! The Practice menu, laid out like the original's "Select Item to
//! Practice" (positions from a capture, estimated): a grid of twenty icons
//! for `LEVEL.080`–`099` plus EXIT. The skill icons are the panel's
//! `MINILEMM` lemmings; the others come from the panel icons, `DEFLICON`
//! and `PRACICON`. The hovered icon animates, its name shows at the bottom
//! and its voice plays (`SOUND/SAMPLIST.TXT`); a green tick marks levels
//! already completed.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use l3d_formats::icons::{Icons, panel as icon};
use l3d_formats::image::IndexedImage;
use l3d_formats::sheets;

use crate::menu::AppState;
use crate::settings::Settings;
use crate::title::{Art, ScreenRoot, at, image_at, spawn_screen};
use crate::{CurrentLevel, Data};

pub struct PracticePlugin;

impl Plugin for PracticePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load)
            .add_systems(Update, (spawn_grid, hover, pick).chain().run_if(in_state(AppState::Practice)))
            .add_systems(OnExit(AppState::Practice), crate::title::despawn::<ScreenRoot>);
    }
}

/// Where an icon's frames come from.
#[derive(Clone, Copy)]
enum Source {
    Mini(usize),
    Panel(usize),
    Deflicon,
    Prac(usize, usize),
}

/// The first Practice level.
const FIRST: u32 = 80;
const ROW_Y: [f32; 3] = [37.0, 86.0, 136.0];
const COLUMNS: [[f32; 7]; 3] =
    [[22.0, 70.0, 115.0, 160.0, 200.0, 238.0, 277.0], [39.0, 78.0, 120.0, 160.0, 200.0, 238.0, 277.0], [36.0, 81.0, 118.0, 160.0, 205.0, 241.0, 281.0]];
/// Icons in grid order: levels 80–99, then EXIT; each with its voice.
const ITEMS: [(Source, &str); 21] = [
    (Source::Mini(19), "VOXFX/BLOCKER1"),
    (Source::Mini(26), "VOXFX/TURNER1"),
    (Source::Mini(0), "VOXFX/BOMBER"),
    (Source::Mini(46), "VOXFX/BUILDER2"),
    (Source::Mini(33), "VOXFX/BASHER2"),
    (Source::Mini(52), "VOXFX/MINER2"),
    (Source::Mini(58), "VOXFX/DIGGER2"),
    (Source::Mini(11), "VOXFX/CLIMBER2"),
    (Source::Mini(41), "VOXFX/FLOATER1"),
    (Source::Panel(icon::FACE + 1), "VOXFX/HILITE"),
    (Source::Panel(icon::RED_DOWN), "VOXFX/VIRTUAL1"),
    (Source::Deflicon, "VOXFX/DEFLECT"),
    (Source::Prac(26, 8), "VOXFX/MUD1"),
    (Source::Prac(6, 4), "VOXFX/ONEWAY"),
    (Source::Prac(4, 2), "VOXFX/SPLITTER"),
    (Source::Prac(34, 5), "VOXFX/SLIPPER1"),
    (Source::Prac(10, 5), "VOXFX/ROCKSL1"),
    (Source::Prac(19, 7), "VOXFX/SPRING"),
    (Source::Prac(15, 4), "VOXFX/TRAMPOL1"),
    (Source::Prac(0, 4), "VOXFX/TELEPORT"),
    (Source::Prac(40, 16), "VOXFX/EXIT"),
];
const TICK: usize = 39;
const FPS: f32 = 12.0;

/// Uploaded frames per item, and the tick.
#[derive(Resource)]
struct Pics {
    items: Vec<Vec<Handle<Image>>>,
    tick: Handle<Image>,
}

fn upload(img: &IndexedImage, pal: &l3d_formats::gamedata::Palette, images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.to_rgba(pal, true),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

fn load(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else { return };
    let mut cut = |s: &sheets::Sheet| d.read(s.path).ok().and_then(|raw| s.cut(&raw).ok()).unwrap_or_default();
    let (mini, defl, prac) = (cut(&sheets::MINILEMM), cut(&sheets::DEFLICON), cut(&sheets::PRACICON));
    let panel = d.read("GFX/ICONS.RNC").ok().and_then(|raw| Icons::parse(&raw).ok()).map(|i| i.panel).unwrap_or_default();
    if mini.is_empty() || prac.len() <= TICK {
        return;
    }
    let mut up = |imgs: &[IndexedImage]| imgs.iter().map(|i| upload(i, &pal, &mut images)).collect::<Vec<_>>();
    let items = ITEMS
        .iter()
        .map(|(src, _)| match *src {
            Source::Mini(c) => up(&mini[c..=c]),
            Source::Panel(c) => up(&panel[c.min(panel.len().saturating_sub(1))..=c.min(panel.len().saturating_sub(1))]),
            Source::Deflicon => up(&defl),
            Source::Prac(first, n) => up(&prac[first..first + n]),
        })
        .collect();
    let tick = up(&prac[TICK..=TICK]).remove(0);
    commands.insert_resource(Pics { items, tick });
}

/// An icon: its index in [`ITEMS`].
#[derive(Component, Clone, Copy)]
struct Item(usize);

/// The hovered item's name.
#[derive(Component)]
struct NameLine;

fn spawn_grid(mut commands: Commands, art: Option<Res<Art>>, pics: Option<Res<Pics>>, settings: Res<Settings>, roots: Query<(), With<ScreenRoot>>) {
    let (Some(art), Some(pics)) = (art, pics) else { return };
    if !roots.is_empty() {
        return;
    }
    let canvas = spawn_screen(&mut commands, &art, 0.0, Color::srgb(0.75, 0.75, 0.75));
    if let Some(font) = &art.large {
        let (glyphs, w) = font.layout("Select Item to Practice");
        let left = (160.0 - w / 2.0).round();
        for (x, image) in glyphs {
            let e = commands.spawn(image_at(image, at(left + x, 12.0, font.size.x, font.size.y))).id();
            commands.entity(canvas).add_child(e);
        }
    }
    for (i, frames) in pics.items.iter().enumerate() {
        let (row, col) = (i / 7, i % 7);
        let (cx, cy) = (COLUMNS[row][col], ROW_Y[row] + 16.0);
        let size = if matches!(ITEMS[i].0, Source::Panel(_)) { 24.0 } else { 32.0 };
        let e = commands.spawn((Item(i), Button, image_at(frames[0].clone(), at(cx - size / 2.0, cy - size / 2.0, size, size)))).id();
        commands.entity(canvas).add_child(e);
        if i < 20 && settings.completed.contains(&(FIRST + i as u32)) {
            let t = commands.spawn((image_at(pics.tick.clone(), at(cx - 4.0, cy - 16.0, 32.0, 32.0)), Pickable::IGNORE)).id();
            commands.entity(canvas).add_child(t);
        }
    }
    let line = commands.spawn((NameLine, Node { position_type: PositionType::Absolute, ..default() }, at(0.0, 0.0, 320.0, 200.0), Pickable::IGNORE)).id();
    commands.entity(canvas).add_child(line);
}

/// Animates the hovered icon, names it and says it.
#[allow(clippy::too_many_arguments)]
fn hover(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<Art>>,
    pics: Option<Res<Pics>>,
    titles: Option<Res<crate::menu::Titles>>,
    mut items: Query<(&Item, &Interaction, &mut ImageNode)>,
    lines: Query<(Entity, Option<&Children>), With<NameLine>>,
    mut last: Local<Option<usize>>,
    mut sfx: MessageWriter<crate::sfx::Sfx>,
) {
    let (Some(pics), Some(art)) = (pics, art) else { return };
    let mut hovered = None;
    for (item, interaction, mut node) in &mut items {
        let frames = &pics.items[item.0];
        let frame = if *interaction != Interaction::None {
            hovered = Some(item.0);
            (time.elapsed_secs() * FPS) as usize % frames.len()
        } else {
            0
        };
        if node.image != frames[frame] {
            node.image = frames[frame].clone();
        }
    }
    if hovered == *last {
        return;
    }
    *last = hovered;
    let Ok((line, children)) = lines.single() else { return };
    for c in children.into_iter().flatten() {
        commands.entity(*c).despawn();
    }
    let Some(i) = hovered else { return };
    sfx.write(crate::sfx::Sfx(ITEMS[i].1));
    let name = if i < 20 { titles.map(|t| crate::menu::title_case(&t.0[(FIRST as usize) + i])).unwrap_or_default() } else { "Exit".into() };
    if let Some(font) = &art.large {
        let (glyphs, w) = font.layout(&name);
        let left = (160.0 - w / 2.0).round();
        for (x, image) in glyphs {
            let e = commands.spawn(image_at(image, at(left + x, 178.0, font.size.x, font.size.y))).id();
            commands.entity(line).add_child(e);
        }
    }
}

fn pick(
    items: Query<(&Item, &Interaction), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
        return;
    }
    for (item, interaction) in &items {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if item.0 == 20 {
            next.set(AppState::Title);
        } else {
            current.number = FIRST + item.0 as u32;
            current.loaded = None;
            next.set(AppState::Briefing);
        }
    }
}
