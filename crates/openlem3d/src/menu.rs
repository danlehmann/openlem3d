//! Level selection: every level is available from the start, grouped by
//! difficulty rating (`docs/spec/level.md` for the file numbering).

use bevy::prelude::*;

use crate::{CurrentLevel, Data};

/// Whether the player is choosing a level or playing one.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// The title screen (main menu).
    #[default]
    Title,
    /// Entering a level code.
    Code,
    /// The configuration screen.
    Options,
    /// The Practice menu.
    Practice,
    /// Choosing a level.
    Menu,
    /// The level's briefing, before it starts.
    Briefing,
    Playing,
    /// The end-of-level screen.
    Results,
}

/// The difficulty ratings in file order; level `n` (1-based) of rating `r`
/// is `LEVEL.(20·r + n − 1)`.
pub const RATINGS: [&str; 5] = ["Fun", "Tricky", "Taxing", "Mayhem", "Practice"];
const LEVELS_PER_RATING: u32 = 20;
/// The Practice rating's index in [`RATINGS`].
pub const PRACTICE: usize = 4;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<MenuRating>()
            .add_systems(Startup, read_titles)
            .init_resource::<Scroll>()
            .add_systems(Startup, load_labels)
            .add_systems(Update, (ensure_menu, menu_input, menu_to_title).chain().run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .add_systems(Update, back_to_menu.run_if(in_state(AppState::Playing)));
    }
}

/// Level titles by file number.
#[derive(Resource)]
pub(crate) struct Titles(pub(crate) Vec<String>);

/// The rating whose levels the menu lists.
#[derive(Resource, Default)]
pub struct MenuRating(pub usize);

fn read_titles(mut commands: Commands, mut data: ResMut<Data>) {
    let titles = (0..RATINGS.len() as u32 * LEVELS_PER_RATING)
        .map(|n| data.0.level(n).map(|l| l.title.trim().to_string()).unwrap_or_else(|_| format!("LEVEL.{n:03}")))
        .collect();
    commands.insert_resource(Titles(titles));
}

/// Formats an all-caps title in title case ("THAT'S RIGHT" → "That's Right").
pub(crate) fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The level list, laid out like the original's "Select Fun Level To Play"
/// (from a capture; positions estimated, in 320×200): the title in white,
/// one red row per level ("1  Take A Dive"), and the best result under the
/// IN and clock icons on the right. The original lists only unlocked
/// levels; all twenty are open here, so the list scrolls (wheel, arrow
/// keys or Page Up/Down). Left and Right change the rating.
#[derive(Component)]
struct ListRow(u32);

/// The scrolling part of the list.
#[derive(Component)]
struct ListBody;

/// List geometry: first row, row spacing, rows visible.
const ROW_TOP: f32 = 38.0;
const ROW_STEP: f32 = 15.0;
const ROWS_SHOWN: usize = 10;
const RED: Color = Color::srgb(1.0, 0.1, 0.1);
const HOVER: Color = Color::srgb(1.0, 0.9, 0.2);

/// How far the list is scrolled, in rows.
#[derive(Resource, Default)]
struct Scroll(usize);

/// The IN and clock labels.
#[derive(Resource)]
struct ListLabels(Handle<Image>, Handle<Image>);

fn load_labels(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let d = &mut data.0;
    let (Ok(pal), Some(icons)) = (d.palette("GFX/LM3D.PAL"), d.read("GFX/ICONS.RNC").ok().and_then(|r| l3d_formats::icons::Icons::parse(&r).ok())) else { return };
    let mut up = |img: &l3d_formats::image::IndexedImage| {
        let mut image = Image::new(
            Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            img.to_rgba(&pal, true),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        images.add(image)
    };
    if let (Some(inl), Some(clock)) = (icons.labels.first(), icons.labels.get(2)) {
        commands.insert_resource(ListLabels(up(inl), up(clock)));
    }
}

/// Builds the list when it is shown and doesn't exist yet.
#[allow(clippy::too_many_arguments)]
fn ensure_menu(
    mut commands: Commands,
    art: Option<Res<crate::title::Art>>,
    labels: Option<Res<ListLabels>>,
    titles: Option<Res<Titles>>,
    rating: Res<MenuRating>,
    settings: Res<crate::settings::Settings>,
    scroll: Res<Scroll>,
    roots: Query<(), With<crate::title::ScreenRoot>>,
) {
    use crate::title::{at, image_at, spawn_screen};
    let (Some(art), Some(labels), Some(titles)) = (art, labels, titles) else { return };
    if !roots.is_empty() {
        return;
    }
    let Some(font) = art.large.as_ref() else { return };
    let canvas = spawn_screen(&mut commands, &art, 0.0, Color::srgb(0.75, 0.75, 0.75));
    let text = |commands: &mut Commands, parent: Entity, s: &str, x: f32, y: f32, right: bool, colour: Color, row: Option<u32>| {
        let (glyphs, w) = font.layout(s);
        let left = if right { x - w } else { x };
        for (gx, image) in glyphs {
            let (_, a, node) = image_at(image.clone(), at(left + gx, y, font.size.x, font.size.y));
            let mut e = commands.spawn((ImageNode::new(image).with_color(colour), a, node));
            if let Some(n) = row {
                e.insert(ListRow(n));
            }
            let id = e.id();
            commands.entity(parent).add_child(id);
        }
    };
    let heading = format!("Select {} Level To Play", RATINGS[rating.0]);
    let (_, w) = font.layout(&heading);
    text(&mut commands, canvas, &heading, (160.0 - w / 2.0).round(), 4.0, false, Color::WHITE, None);
    for (img, x) in [(&labels.0, 246.0), (&labels.1, 287.0)] {
        let e = commands.spawn(image_at(img.clone(), at(x, 18.0, 32.0, 16.0))).id();
        commands.entity(canvas).add_child(e);
    }
    // Rows clipped to the list area.
    let window = commands
        .spawn((at(0.0, ROW_TOP, 320.0, ROW_STEP * ROWS_SHOWN as f32), Node { position_type: PositionType::Absolute, overflow: Overflow::clip(), ..default() }))
        .id();
    commands.entity(canvas).add_child(window);
    let body = commands.spawn((ListBody, at(0.0, -(scroll.0 as f32) * ROW_STEP, 320.0, ROW_STEP * LEVELS_PER_RATING as f32), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(window).add_child(body);
    for i in 0..LEVELS_PER_RATING {
        let n = rating.0 as u32 * LEVELS_PER_RATING + i;
        let y = i as f32 * ROW_STEP;
        // An invisible button spanning the row.
        let hit = commands.spawn((Button, ListRow(n), at(0.0, y, 320.0, ROW_STEP), Node { position_type: PositionType::Absolute, ..default() })).id();
        commands.entity(body).add_child(hit);
        text(&mut commands, body, &(i + 1).to_string(), 22.0, y, true, RED, Some(n));
        text(&mut commands, body, &title_case(&titles.0[n as usize]), 32.0, y, false, RED, Some(n));
        let (saved, secs) = settings.best.get(&n).copied().unwrap_or((0, 0));
        text(&mut commands, body, &saved.to_string(), 270.0, y, true, RED, Some(n));
        text(&mut commands, body, &format!("{}:{:02}", secs / 60, secs % 60), 316.0, y, true, RED, Some(n));
    }
}

fn despawn_menu(mut commands: Commands, roots: Query<Entity, With<crate::title::ScreenRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Clicks, hover colour, scrolling and rating changes.
#[allow(clippy::too_many_arguments)]
fn menu_input(
    mut commands: Commands,
    rows: Query<(&Interaction, &ListRow), With<Button>>,
    mut glyphs: Query<(&ListRow, &mut ImageNode)>,
    mut bodies: Query<&mut crate::title::At, With<ListBody>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    mut drag: Local<f32>,
    mut scroll: ResMut<Scroll>,
    mut rating: ResMut<MenuRating>,
    roots: Query<Entity, With<crate::title::ScreenRoot>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    let max = LEVELS_PER_RATING as usize - ROWS_SHOWN;
    let mut delta: i32 = wheel.read().map(|w| if w.y > 0.0 { -1 } else if w.y < 0.0 { 1 } else { 0 }).sum();
    if keys.just_pressed(KeyCode::ArrowDown) {
        delta += 1;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        delta -= 1;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        delta += ROWS_SHOWN as i32;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= ROWS_SHOWN as i32;
    }
    // Dragging a finger up or down scrolls a row per row height moved.
    let row_px = ROW_STEP * windows.single().map_or(4.0, |w| (w.width() / 320.0).min(w.height() / 200.0));
    for t in touches.iter() {
        *drag -= t.delta().y;
    }
    while drag.abs() >= row_px {
        delta += drag.signum() as i32;
        *drag -= drag.signum() * row_px;
    }
    if touches.iter().next().is_none() {
        *drag = 0.0;
    }
    if delta != 0 {
        scroll.0 = (scroll.0 as i32 + delta).clamp(0, max as i32) as usize;
        for mut a in &mut bodies {
            *a = crate::title::at(0.0, -(scroll.0 as f32) * ROW_STEP, 320.0, ROW_STEP * LEVELS_PER_RATING as f32);
        }
    }
    // Left and Right step through the ratings (Practice has its own menu).
    let step = keys.just_pressed(KeyCode::ArrowRight) as i32 - keys.just_pressed(KeyCode::ArrowLeft) as i32;
    if step != 0 {
        rating.0 = (rating.0 as i32 + step).rem_euclid(PRACTICE as i32) as usize;
        scroll.0 = 0;
        for e in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    let hovered = rows.iter().find(|(i, _)| **i != Interaction::None).map(|(_, r)| r.0);
    for (row, mut node) in &mut glyphs {
        let colour = if Some(row.0) == hovered { HOVER } else { RED };
        if node.color != colour && node.image != Handle::default() {
            node.color = colour;
        }
    }
    if let Some((_, row)) = rows.iter().find(|(i, _)| **i == Interaction::Pressed) {
        current.number = row.0;
        current.loaded = None;
        next.set(AppState::Briefing);
    }
}
pub(crate) fn back_to_menu(keys: Res<ButtonInput<KeyCode>>, current: Res<crate::CurrentLevel>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(level_menu(current.number));
    }
}

/// The menu a level belongs to: Practice levels have their own.
pub fn level_menu(level: u32) -> AppState {
    if level >= PRACTICE as u32 * LEVELS_PER_RATING { AppState::Practice } else { AppState::Menu }
}

/// Esc on the level-select screen returns to the title screen.
fn menu_to_title(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
}
