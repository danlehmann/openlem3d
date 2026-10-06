//! Level selection: every level is available from the start, grouped by
//! difficulty rating (`docs/spec/level.md` for the file numbering).

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::{CurrentLevel, Data};

/// Whether the player is choosing a level or playing one.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// The title screen (main menu).
    #[default]
    Title,
    /// Entering a level code.
    Code,
    /// Choosing a level.
    Menu,
    Playing,
}

/// The difficulty ratings in file order; level `n` (1-based) of rating `r`
/// is `LEVEL.(20·r + n − 1)`.
pub const RATINGS: [&str; 5] = ["Fun", "Tricky", "Taxing", "Mayhem", "Practice"];
const LEVELS_PER_RATING: u32 = 20;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<MenuRating>()
            .add_systems(Startup, read_titles)
            .add_systems(Update, ensure_menu.run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .init_resource::<CodeEntry>()
            .add_systems(
                Update,
                (menu_buttons, rebuild_on_rating_change, code_input, menu_to_title).run_if(in_state(AppState::Menu)),
            )
            .add_systems(Update, back_to_menu.run_if(in_state(AppState::Playing)));
    }
}

/// Level titles by file number.
#[derive(Resource)]
struct Titles(Vec<String>);

/// The rating whose levels the menu lists.
#[derive(Resource, Default)]
pub struct MenuRating(pub usize);

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
enum MenuButton {
    Rating(usize),
    Level(u32),
}

fn read_titles(mut commands: Commands, mut data: ResMut<Data>) {
    let titles = (0..RATINGS.len() as u32 * LEVELS_PER_RATING)
        .map(|n| data.0.level(n).map(|l| l.title.trim().to_string()).unwrap_or_else(|_| format!("LEVEL.{n:03}")))
        .collect();
    commands.insert_resource(Titles(titles));
}

/// Formats an all-caps title in title case ("THAT'S RIGHT" → "That's Right").
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

const PANEL: Color = Color::srgba(0.05, 0.05, 0.12, 0.88);
const BUTTON: Color = Color::srgba(0.15, 0.2, 0.45, 0.95);
const BUTTON_ACTIVE: Color = Color::srgba(0.8, 0.2, 0.1, 0.95);

/// Builds the menu when it is shown and does not exist yet (titles are read
/// at startup, which may complete after the initial state is entered).
fn ensure_menu(
    mut commands: Commands,
    titles: Option<Res<Titles>>,
    rating: Res<MenuRating>,
    roots: Query<(), With<MenuRoot>>,
) {
    if roots.is_empty()
        && let Some(titles) = titles
    {
        build_menu(&mut commands, &titles, rating.0);
    }
}

fn build_menu(commands: &mut Commands, titles: &Titles, rating: usize) {
    let text = |s: String, size: f32| (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() });
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|root| {
            root.spawn(text("openlem3d - choose a level".into(), 26.0));
            root.spawn((text("Type a level code and press Enter".into(), 16.0), CodeText));
            root.spawn(Node { column_gap: px(6), ..default() }).with_children(|tabs| {
                for (i, name) in RATINGS.iter().enumerate() {
                    tabs.spawn((
                        Button,
                        MenuButton::Rating(i),
                        Node { padding: UiRect::axes(px(14), px(8)), ..default() },
                        BackgroundColor(if i == rating { BUTTON_ACTIVE } else { BUTTON }),
                    ))
                    .with_child(text(name.to_string(), 18.0));
                }
            });
            root.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                column_gap: px(6),
                row_gap: px(4),
                width: percent(100),
                max_width: px(900),
                ..default()
            })
            .with_children(|grid| {
                for i in 0..LEVELS_PER_RATING {
                    let n = rating as u32 * LEVELS_PER_RATING + i;
                    grid.spawn((
                        Button,
                        MenuButton::Level(n),
                        Node { padding: UiRect::axes(px(10), px(6)), ..default() },
                        BackgroundColor(BUTTON),
                    ))
                    .with_child(text(format!("{:2}  {}", i + 1, title_case(&titles.0[n as usize])), 16.0));
                }
            });
        });
}

fn despawn_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn menu_buttons(
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut rating: ResMut<MenuRating>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            MenuButton::Rating(r) => rating.0 = r,
            MenuButton::Level(n) => {
                current.number = n;
                current.loaded = None;
                next.set(AppState::Playing);
            }
        }
    }
}

/// Re-creates the menu when another rating tab is chosen.
fn rebuild_on_rating_change(
    rating: Res<MenuRating>,
    mut commands: Commands,
    roots: Query<Entity, With<MenuRoot>>,
    titles: Option<Res<Titles>>,
) {
    if !rating.is_changed() || rating.is_added() {
        return;
    }
    let Some(titles) = titles else { return };
    for e in &roots {
        commands.entity(e).despawn();
    }
    build_menu(&mut commands, &titles, rating.0);
}

/// The level code being typed on the menu.
#[derive(Resource, Default)]
struct CodeEntry(String);

/// The line showing the code being typed.
#[derive(Component)]
struct CodeText;

/// Longest code accepted (the original's codes are short words).
const MAX_CODE_LEN: usize = 12;

/// Typing letters builds a code; Enter jumps to its level.
fn code_input(
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut entry: ResMut<CodeEntry>,
    mut texts: Query<&mut Text, With<CodeText>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    use bevy::input::keyboard::Key;
    let mut message = None;
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Character(s) => {
                for c in s.chars().filter(|c| c.is_ascii_alphanumeric()) {
                    if entry.0.len() < MAX_CODE_LEN {
                        entry.0.push(c.to_ascii_uppercase());
                    }
                }
            }
            Key::Backspace => {
                entry.0.pop();
            }
            Key::Enter if !entry.0.is_empty() => match crate::codes::level_for_code(&entry.0) {
                Some(n) => {
                    current.number = n;
                    current.loaded = None;
                    entry.0.clear();
                    next.set(AppState::Playing);
                }
                None => {
                    message = Some(format!("Unknown code {}", entry.0));
                    entry.0.clear();
                }
            },
            _ => {}
        }
    }
    if entry.is_changed() || message.is_some() {
        let line = message.unwrap_or_else(|| {
            if entry.0.is_empty() { "Type a level code and press Enter".into() } else { format!("Code: {}", entry.0) }
        });
        for mut t in &mut texts {
            t.0 = line.clone();
        }
    }
}

fn back_to_menu(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Menu);
    }
}

/// Esc on the level-select screen returns to the title screen.
fn menu_to_title(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
}
