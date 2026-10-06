//! The end-of-level screen: the level's theme picture, how many lemmings
//! were saved against how many were needed, the next level's password when
//! known, and two choices: Next Level (or Retry after a failure) on the left
//! button and Menu on the right, as in the original (navigation notes:
//! "Next Level"/"Retry" bottom left, "Menu" bottom right). The wording of
//! the result lines is our own. Input is taken at once.

use bevy::prelude::*;

use crate::menu::AppState;
use crate::title::{Art, Canvas, at, image_at};
use crate::{CurrentLevel, Data, Game};

pub struct ResultsPlugin;

impl Plugin for ResultsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, level_over.run_if(in_state(AppState::Playing)))
            .add_systems(Update, (spawn_results, results_input).chain().run_if(in_state(AppState::Results)))
            .add_systems(OnExit(AppState::Results), despawn_results);
    }
}

/// How the last level went.
#[derive(Resource, Clone, Copy)]
pub struct Outcome {
    level: u32,
    saved: u32,
    total: u32,
    needed: u32,
}

impl Outcome {
    fn passed(&self) -> bool {
        self.saved >= self.needed
    }
}

const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const TEXT_SCALE: f32 = 1.5;
/// Lemming levels in the game.
const LEVELS: u32 = 100;

/// Moves to the results once the level is over.
fn level_over(mut commands: Commands, game: Res<Game>, current: Res<CurrentLevel>, mut next: ResMut<NextState<AppState>>) {
    let Some(sim) = &game.sim else { return };
    // Until the chosen level has loaded, the simulation is the last one's.
    if current.loaded != Some(current.number) || !sim.finished() || game.terrain.is_none() {
        return;
    }
    commands.insert_resource(Outcome { level: current.number, saved: sim.counts.saved, total: sim.to_release.max(1), needed: game.save_requirement });
    next.set(AppState::Results);
}

#[derive(Component)]
struct ResultsRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Next,
    Menu,
}

fn spawn_results(
    mut commands: Commands,
    art: Option<Res<Art>>,
    outcome: Option<Res<Outcome>>,
    mut data: ResMut<Data>,
    mut images: ResMut<Assets<Image>>,
    roots: Query<(), With<ResultsRoot>>,
) {
    let Some(o) = outcome.map(|o| *o) else { return };
    if !roots.is_empty() {
        return;
    }
    let Ok(level) = data.0.level(o.level) else { return };
    let pics = crate::briefing::load_scene(&mut data, level.theme, &mut images);
    let root = commands
        .spawn((
            ResultsRoot,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            BackgroundColor(Color::BLACK),
        ))
        .id();
    let canvas = commands.spawn((Canvas(SCREEN), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(root).add_child(canvas);
    let add = |commands: &mut Commands, bundle: (ImageNode, crate::title::At, Node)| {
        let e = commands.spawn(bundle).id();
        commands.entity(canvas).add_child(e);
    };
    if let Some(p) = &pics {
        add(&mut commands, image_at(p.picture.clone(), at(0.0, 0.0, SCREEN.x, SCREEN.y)));
        // A dark panel behind the result lines.
        let panel = commands
            .spawn((at(120.0, 120.0, 400.0, 200.0), Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))))
            .id();
        commands.entity(canvas).add_child(panel);
        for (i, x) in [(0, 25.0), (1, 522.0)] {
            if let Some(h) = p.prompts.get(i).cloned() {
                add(&mut commands, image_at(h, at(x, 438.0, 32.0, 32.0)));
            }
        }
    }
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else { return };
    let text = |commands: &mut Commands, s: &str, x: f32, y: f32, centred: bool| {
        let (glyphs, width) = font.layout(s);
        let left = if centred { x - width * TEXT_SCALE / 2.0 } else { x };
        for (gx, image) in glyphs {
            add(commands, image_at(image, at(left + gx * TEXT_SCALE, y, font.size.x * TEXT_SCALE, font.size.y * TEXT_SCALE)));
        }
    };
    let pct = |n: u32| n * 100 / o.total;
    text(&mut commands, &format!("Level {}  {}", o.level + 1, level.title.trim()), 21.0, 6.0, false);
    let verdict = match (o.passed(), o.saved == o.total) {
        (true, true) => "Superb! Everyone saved!",
        (true, false) => "Well done!",
        (false, _) => "Not enough lemmings saved",
    };
    text(&mut commands, verdict, 320.0, 140.0, true);
    text(&mut commands, &format!("You rescued {}%", pct(o.saved)), 320.0, 180.0, true);
    text(&mut commands, &format!("You needed {}%", pct(o.needed)), 320.0, 210.0, true);
    let next_level = if o.passed() { (o.level + 1) % LEVELS } else { o.level };
    if o.passed()
        && let Some(code) = crate::codes::code_for_level(next_level)
    {
        text(&mut commands, &format!("Password :- {code}"), 320.0, 260.0, true);
    }
    text(&mut commands, if o.passed() { "Next Level" } else { "Retry" }, 61.0, 445.0, false);
    text(&mut commands, "Menu", 561.0, 445.0, false);
    for (choice, x, w) in [(Choice::Next, 25.0, 200.0), (Choice::Menu, 522.0, 110.0)] {
        let e = commands.spawn((choice, Button, at(x, 438.0, w, 32.0), Node { position_type: PositionType::Absolute, ..default() })).id();
        commands.entity(canvas).add_child(e);
    }
}

fn despawn_results(mut commands: Commands, roots: Query<Entity, With<ResultsRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Left click, Space or Enter: the next level (or a retry); right click or
/// Esc: the level list.
fn results_input(
    choices: Query<(&Interaction, &Choice)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    outcome: Option<Res<Outcome>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(o) = outcome else { return };
    let on = |c: Choice| choices.iter().any(|(i, k)| *i == Interaction::Pressed && *k == c);
    let on_prompt = choices.iter().any(|(i, _)| *i != Interaction::None);
    let menu = mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) || on(Choice::Menu);
    let go = on(Choice::Next)
        || (mouse.just_pressed(MouseButton::Left) && !on_prompt)
        || (touches.any_just_released() && !on_prompt)
        || keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::NumpadEnter]);
    if menu {
        next.set(AppState::Menu);
    } else if go {
        current.number = if o.passed() { (o.level + 1) % LEVELS } else { o.level };
        current.loaded = None;
        next.set(AppState::Briefing);
    }
}
