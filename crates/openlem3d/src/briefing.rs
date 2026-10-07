//! The briefing shown before a level, like the original's: the level's
//! theme picture (`SCENEnnn`, 640×480) with "Level N  TITLE" across the top
//! and the choices along the bottom: Continue (left click or Space), Preview
//! (Enter) and Menu (right click or Esc). It takes input from its first
//! frame (`docs/GROUNDRULES.md`). Positions are estimated from captures of
//! the original's briefings.
//!
//! Preview starts the level paused with the camera circling the level's
//! preview pivot; any key or click ends it and the level runs.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use l3d_formats::image::IndexedImage;
use l3d_formats::screen;

use crate::menu::AppState;
use crate::title::{Art, At, Canvas, at, image_at};
use crate::{CurrentLevel, Data, Game, ViewCamera};

pub struct BriefingPlugin;

impl Plugin for BriefingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Preview>()
            .add_systems(Update, (spawn_briefing, briefing_input).chain().run_if(in_state(AppState::Briefing)))
            .add_systems(OnExit(AppState::Briefing), despawn_briefing)
            .add_systems(Update, preview.before(crate::camera_controls).run_if(in_state(AppState::Playing)))
            .add_systems(
                Update,
                end_demo
                    .before(crate::menu::back_to_menu)
                    .before(crate::hud::assign_on_pointer)
                    .before(crate::lemming_cam::toggle_keys)
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

/// The briefing canvas: the scene pictures' size.
const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
/// Text is the panel's large font at this scale (estimated from captures).
const TEXT_SCALE: f32 = 1.5;
const TOP_Y: f32 = 6.0;
const PROMPT_Y: f32 = 438.0;
const LABEL_Y: f32 = 445.0;

#[derive(Component)]
struct BriefingRoot;

/// A theme's picture and its four prompt cells, uploaded.
#[derive(Clone)]
pub(crate) struct ScenePics {
    pub(crate) picture: Handle<Image>,
    pub(crate) prompts: Vec<Handle<Image>>,
}

fn upload(img: &IndexedImage, pal: &l3d_formats::gamedata::Palette, images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.to_rgba(pal, false),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

/// Loads `SCENEnnn.SVG` (640×480) with its palette.
pub(crate) fn load_scene(data: &mut Data, theme: u8, images: &mut Assets<Image>) -> Option<ScenePics> {
    let d = &mut data.0;
    let scene = screen::scene(&d.read(&format!("GFX/SCENE{theme:03}.SVG")).ok()?, screen::HIGH_RES).ok()?;
    let pal = d.palette(&format!("GFX/SCENE{theme:03}.SVP")).ok()?;
    Some(ScenePics {
        picture: upload(&scene.image, &pal, images),
        prompts: scene.prompts.iter().map(|p| upload(p, &pal, images)).collect(),
    })
}

/// Builds the briefing when it is shown and doesn't exist yet.
#[allow(clippy::too_many_arguments)]
fn spawn_briefing(
    mut commands: Commands,
    art: Option<Res<Art>>,
    current: Res<CurrentLevel>,
    mut data: ResMut<Data>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u8, Option<ScenePics>>>,
    roots: Query<(), With<BriefingRoot>>,
) {
    if !roots.is_empty() {
        return;
    }
    let Ok(level) = data.0.level(current.number) else { return };
    let pics = cache.entry(level.theme).or_insert_with(|| load_scene(&mut data, level.theme, &mut images)).clone();
    let root = commands
        .spawn((
            BriefingRoot,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            BackgroundColor(Color::BLACK),
        ))
        .id();
    let canvas = commands.spawn((Canvas(SCREEN), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(root).add_child(canvas);
    let add = |commands: &mut Commands, bundle: (ImageNode, At, Node)| {
        let e = commands.spawn(bundle).id();
        commands.entity(canvas).add_child(e);
    };
    if let Some(p) = &pics {
        add(&mut commands, image_at(p.picture.clone(), at(0.0, 0.0, SCREEN.x, SCREEN.y)));
        // Left button, right button, the two halves of the ENTER key.
        let prompt = |i: usize| p.prompts.get(i).cloned();
        for (i, x) in [(0, 25.0), (2, 268.0), (3, 300.0), (1, 522.0)] {
            if let Some(h) = prompt(i) {
                add(&mut commands, image_at(h, at(x, PROMPT_Y, 32.0, 32.0)));
            }
        }
    }
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else { return };
    let text = |commands: &mut Commands, s: &str, x: f32, y: f32, right_aligned: bool| {
        let (glyphs, width) = font.layout(s);
        let left = if right_aligned { x - width * TEXT_SCALE } else { x };
        for (gx, image) in glyphs {
            let r = at(left + gx * TEXT_SCALE, y, font.size.x * TEXT_SCALE, font.size.y * TEXT_SCALE);
            add(commands, image_at(image, r));
        }
    };
    text(&mut commands, "Level", 21.0, TOP_Y, false);
    text(&mut commands, &(current.number + 1).to_string(), 158.0, TOP_Y, true);
    text(&mut commands, level.title.trim(), 169.0, TOP_Y, false);
    text(&mut commands, "Continue", 61.0, LABEL_Y, false);
    text(&mut commands, if has_demo(current.number) { "Demo" } else { "Preview" }, 342.0, LABEL_Y, false);
    text(&mut commands, "Menu", 561.0, LABEL_Y, false);
    // Clicking or tapping a prompt picks it; anywhere else continues.
    for (choice, x, w) in [(Choice::Preview, 268.0, 150.0), (Choice::Menu, 522.0, 110.0)] {
        let e = commands.spawn((choice, Button, at(x, PROMPT_Y, w, 32.0), Node { position_type: PositionType::Absolute, ..default() })).id();
        commands.entity(canvas).add_child(e);
    }
}

/// A clickable prompt.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Preview,
    Menu,
}

fn despawn_briefing(mut commands: Commands, roots: Query<Entity, With<BriefingRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn briefing_input(
    choices: Query<(&Interaction, &Choice)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut preview: ResMut<Preview>,
    current: Res<CurrentLevel>,
    mut data: ResMut<Data>,
    mut game: ResMut<Game>,
    mut next: ResMut<NextState<AppState>>,
) {
    let on = |c: Choice| choices.iter().any(|(i, k)| *i == Interaction::Pressed && *k == c);
    let on_prompt = choices.iter().any(|(i, _)| *i != Interaction::None);
    let back = mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) || on(Choice::Menu);
    let show = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || on(Choice::Preview);
    let go = (mouse.just_pressed(MouseButton::Left) && !on_prompt)
        || keys.just_pressed(KeyCode::Space)
        || (touches.any_just_released() && !on_prompt)
        || keys.get_just_pressed().any(|k| !matches!(k, KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter));
    if back {
        next.set(crate::menu::level_menu(current.number));
    } else if show && has_demo(current.number) {
        let n = current.number;
        let log = data.0.level(n).ok().zip(data.0.blocks(n).ok()).and_then(|(level, blocks)| l3d_sim::demos::demo(n, &level, &blocks));
        game.replay = log.map(|log| crate::Replay::demo(log, AppState::Briefing));
        next.set(AppState::Playing);
    } else if show {
        preview.active = true;
        preview.started = false;
        next.set(AppState::Playing);
    } else if go {
        next.set(AppState::Playing);
    }
}

/// The level preview: the camera circles the pivot while the level waits.
#[derive(Resource, Default)]
pub struct Preview {
    pub active: bool,
    started: bool,
    angle: f32,
}

/// Preview orbit: radius and height above the pivot (grid units), and turn
/// speed (radians per second). The original's preview path is unmeasured.
const ORBIT_RADIUS: f32 = 14.0;
const ORBIT_HEIGHT: f32 = 4.0;
const ORBIT_SPEED: f32 = 0.35;

#[allow(clippy::too_many_arguments)]
fn preview(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    current: Res<CurrentLevel>,
    mut state: ResMut<Preview>,
    mut game: ResMut<Game>,
    mut views: Query<&mut ViewCamera>,
    mut pivot: Local<Option<(u32, Vec3)>>,
) {
    if !state.active {
        return;
    }
    let any = keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() || touches.any_just_released();
    if state.started && any {
        state.active = false;
        game.paused = false;
        return;
    }
    // Input that started the preview doesn't end it.
    state.started = true;
    game.paused = true;
    if pivot.is_none_or(|(n, _)| n != current.number) {
        // The pivot is stored (y, z, x) in grid cells.
        let Some(p) = game.terrain.as_ref().map(|(level, ..)| level.preview_pivot) else { return };
        *pivot = Some((current.number, Vec3::new(p[2] as f32 + 0.5, p[0] as f32, p[1] as f32 + 0.5)));
    }
    let Some((_, centre)) = *pivot else { return };
    let Ok(mut view) = views.single_mut() else { return };
    state.angle += ORBIT_SPEED * time.delta_secs();
    // The camera looks at the pivot from a point on the circle.
    let offset = Vec3::new(state.angle.cos(), 0.0, state.angle.sin()) * ORBIT_RADIUS;
    view.pos = centre + offset + Vec3::Y * ORBIT_HEIGHT;
    // Yaw convention: forward = (−cos yaw, 0, −sin yaw).
    view.yaw = state.angle;
}

/// Practice levels offer a demo (as the original's "Enter = Demo") instead
/// of the preview.
fn has_demo(level: u32) -> bool {
    level >= crate::menu::PRACTICE as u32 * crate::menu::LEVELS_PER_RATING && l3d_sim::demos::solution(level).is_some()
}

/// Any key, click or tap ends a demo, back to where it was started.
fn end_demo(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut game: ResMut<Game>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(back_to) = game.replay.as_ref().and_then(|r| r.demo) else { return };
    if keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() || touches.any_just_released() {
        keys.clear();
        mouse.clear();
        game.replay = None;
        current.loaded = None;
        next.set(back_to);
    }
}
