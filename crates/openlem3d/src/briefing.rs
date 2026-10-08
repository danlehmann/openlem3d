//! The briefing shown before a level, like the original's: the level's
//! theme picture (`SCENEnnn`, 640×480) with "Level N  TITLE" across the top
//! and the choices along the bottom: Continue (left click or Space), Preview
//! (Enter) and Menu (right click or Esc). It takes input from its first
//! frame (`docs/GROUNDRULES.md`). Positions are estimated from captures of
//! the original's briefings.
//!
//! Preview starts the level paused with the camera circling the level's
//! preview pivot; any key or click ends it and the level runs. A Practice
//! level's briefing is that preview, with Demo (Enter) among its prompts.

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
            .add_systems(Update, preview_overlay.after(preview))
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

fn upload(img: &IndexedImage, pal: &l3d_formats::gamedata::Palette, transparent0: bool, images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.to_rgba(pal, transparent0),
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
        picture: upload(&scene.image, &pal, false, images),
        // The prompts are drawn over the picture, index 0 transparent (as seen
        // in the original: the mice show the picture around them).
        prompts: scene.prompts.iter().map(|p| upload(p, &pal, true, images)).collect(),
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
    mut preview: ResMut<Preview>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !roots.is_empty() {
        return;
    }
    // A Practice briefing is the flyover itself, with the details and the
    // Demo prompt over it, not the theme picture (`docs/spec/camera.md`).
    if has_demo(current.number) {
        *preview = Preview { active: true, practice: true, ..default() };
        next.set(AppState::Playing);
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
    } else if show {
        *preview = Preview { active: true, ..default() };
        next.set(AppState::Playing);
    } else if go {
        next.set(AppState::Playing);
    }
}

/// The level preview: the camera circles the level while it waits.
#[derive(Resource, Default)]
pub struct Preview {
    pub active: bool,
    /// A Practice level's briefing: Enter plays the demo, and the way back
    /// leads to the Practice screen.
    practice: bool,
    started: bool,
    angle: f32,
    /// The camera is gliding from where the flyover left it to camera 1;
    /// the level starts when it arrives.
    gliding: bool,
}

/// Preview orbit, measured in the original (`docs/spec/camera.md`): radius in
/// grid units, and the height used when the level sets no pivot. The turn
/// rate is ours: the original turns 2.8125° per drawn frame (one turn in
/// 1.8 s on a fast machine, far slower on the PCs of its day); 45°/s is about
/// what a 15-frames-per-second machine showed.
const ORBIT_RADIUS: f32 = 32.0;
const UNSET_HEIGHT: f32 = 8.0;
const ORBIT_SPEED: f32 = std::f32::consts::FRAC_PI_4;
/// Level flag: the preview keeps camera 1's view still.
const FLAG_PREVIEW_STATIC: u16 = 0x0040;

/// The point the preview circles and its height: the level's pivot (y, z, x)
/// as grid coordinates, or, when unset (all zero), the middle of the x/z
/// extent of its visible blocks at height 8 (measured on Fun 2 and Fun 10).
fn preview_centre(level: &l3d_formats::level::Level, blocks: &l3d_formats::blk::BlockSet) -> Vec3 {
    let [py, pz, px] = level.preview_pivot;
    if [py, pz, px] != [0, 0, 0] {
        return Vec3::new(px as f32, py as f32, pz as f32);
    }
    let visible = |id: u8| blocks.defs.get(id as usize).is_some_and(|d| !d.is_placeholder() && d.faces.iter().any(|f| f.texture != 0xFF));
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for (x, _, z, b, _) in level.cells() {
        if !b.is_empty() && visible(b.id) {
            lo = lo.min(Vec2::new(x as f32, z as f32));
            hi = hi.max(Vec2::new(x as f32 + 1.0, z as f32 + 1.0));
        }
    }
    let mid = if lo.x <= hi.x { (lo + hi) / 2.0 } else { Vec2::splat(16.0) };
    Vec3::new(mid.x, UNSET_HEIGHT, mid.y)
}

#[allow(clippy::too_many_arguments)]
fn preview(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    choices: Query<(&Interaction, &Choice)>,
    mut current: ResMut<CurrentLevel>,
    mut state: ResMut<Preview>,
    mut game: ResMut<Game>,
    mut data: ResMut<Data>,
    mut views: Query<&mut ViewCamera>,
    mut preset: ResMut<crate::PresetIndex>,
    mut next: ResMut<NextState<AppState>>,
    mut centre: Local<Option<(u32, Vec3, bool)>>,
) {
    if !state.active {
        return;
    }
    let on = |c: Choice| choices.iter().any(|(i, k)| *i == Interaction::Pressed && *k == c);
    // Right click (or Esc) goes back to the level list, as in the original;
    // any other key, click or tap starts the level.
    if state.started && (mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) || on(Choice::Menu)) {
        state.active = false;
        game.paused = false;
        current.loaded = None;
        next.set(crate::menu::level_menu(current.number));
        return;
    }
    // On a Practice level Enter plays the demo from the start; it returns
    // here.
    let enter = [KeyCode::Enter, KeyCode::NumpadEnter];
    if state.started && state.practice && (keys.any_just_pressed(enter) || on(Choice::Preview)) {
        // The key or click that starts the demo doesn't end it.
        keys.reset_all();
        mouse.reset_all();
        let n = current.number;
        let log = data.0.level(n).ok().zip(data.0.blocks(n).ok()).and_then(|(level, blocks)| l3d_sim::demos::demo(n, &level, &blocks));
        state.active = false;
        game.paused = false;
        game.replay = log.map(|log| crate::Replay::demo(log, AppState::Briefing));
        current.loaded = None;
        return;
    }
    let on_prompt = choices.iter().any(|(i, _)| *i != Interaction::None);
    let any = keys.get_just_pressed().next().is_some() || (mouse.get_just_pressed().next().is_some() && !on_prompt) || (touches.any_just_released() && !on_prompt);
    let Ok(mut view) = views.single_mut() else { return };
    // The level starts from camera 1: the camera glides there from where
    // the flyover left it, the level still waiting; input meanwhile skips
    // the rest.
    // (`camera_controls` moves the glide on.)
    if state.gliding {
        if any {
            view.finish_glide();
        }
        if view.glide.is_none() {
            *state = Preview::default();
            game.paused = false;
        }
        return;
    }
    if state.started && any {
        if let Some((level, ..)) = game.terrain.as_ref() {
            view.glide_to_preset(&level.cameras[0]);
            preset.0 = 0;
        }
        state.gliding = true;
        return;
    }
    game.paused = true;
    let first = !state.started;
    // Input that started the preview doesn't end it.
    state.started = true;
    let Some((level, blocks, _)) = game.terrain.as_ref() else { return };
    if centre.is_none_or(|(n, ..)| n != current.number) || first {
        // It starts where camera 1 looks from.
        state.angle = level.cameras[0].rotation as f32 * std::f32::consts::FRAC_PI_2;
        *centre = Some((current.number, preview_centre(level, blocks), level.flags & FLAG_PREVIEW_STATIC != 0));
    }
    let Some((_, c, still)) = *centre else { return };
    if still {
        view.set_preset(&level.cameras[0]);
        return;
    }
    // Turning left all the way round (the facing goes +Z, +X, −Z, −X).
    state.angle -= ORBIT_SPEED * time.delta_secs();
    view.yaw = state.angle;
    // Looking at the centre from the circle; forward = (−cos yaw, 0, −sin yaw).
    let forward = Vec3::new(-state.angle.cos(), 0.0, -state.angle.sin());
    view.pos = c - forward * ORBIT_RADIUS;
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

/// The preview's overlay, as in the original: the level's details in white
/// with a dark shadow at the top left, and Continue (left button) and Menu
/// (right button) along the bottom. The panel is hidden meanwhile.
#[derive(Component)]
struct PreviewRoot;

/// Lines of the preview's details (wording from the original; where the
/// numbers sit in each line is estimated).
fn preview_lines(n: u32, level: &l3d_formats::level::Level) -> [String; 6] {
    let rating = crate::menu::RATINGS.get((n / crate::menu::LEVELS_PER_RATING) as usize).copied().unwrap_or("");
    [
        format!("Level {}  {}", n + 1, level.title.trim()),
        format!("Number Of Lemmings {}", level.lemmings),
        format!("{} To Be Saved", level.save_requirement),
        format!("Release Rate {}", level.release_rate),
        format!("Time {}:{:02} Minutes", level.time_minutes, level.time_seconds),
        format!("Rating {rating}"),
    ]
}

#[allow(clippy::too_many_arguments)]
fn preview_overlay(
    mut commands: Commands,
    state: Res<Preview>,
    art: Option<Res<Art>>,
    current: Res<CurrentLevel>,
    game: Res<Game>,
    mut data: ResMut<Data>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u8, Option<ScenePics>>>,
    roots: Query<Entity, With<PreviewRoot>>,
) {
    if !state.active || state.gliding {
        for e in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    let (Some(font), Some((level, ..))) = (art.as_ref().and_then(|a| a.large.as_ref()), game.terrain.as_ref()) else { return };
    let root = commands
        .spawn((PreviewRoot, Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE))
        .id();
    let canvas = commands.spawn((Canvas(SCREEN), Node { position_type: PositionType::Absolute, ..default() }, Pickable::IGNORE)).id();
    commands.entity(root).add_child(canvas);
    let add = |commands: &mut Commands, mut bundle: (ImageNode, At, Node), shade: Option<Color>| {
        if let Some(c) = shade {
            bundle.0.color = c;
        }
        let e = commands.spawn((bundle, Pickable::IGNORE)).id();
        commands.entity(canvas).add_child(e);
    };
    let text = |commands: &mut Commands, s: &str, x: f32, y: f32| {
        let (glyphs, _) = font.layout(s);
        // The shadow first, then the text.
        for (shadow, dx) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
            for (gx, image) in glyphs.iter() {
                let r = at(x + gx * TEXT_SCALE + dx, y + dx, font.size.x * TEXT_SCALE, font.size.y * TEXT_SCALE);
                add(commands, image_at(image.clone(), r), shadow);
            }
        }
    };
    for (k, line) in preview_lines(current.number, level).iter().enumerate() {
        text(&mut commands, line, 21.0, TOP_Y + 26.0 * k as f32);
    }
    let pics = cache.entry(level.theme).or_insert_with(|| load_scene(&mut data, level.theme, &mut images)).clone();
    if let Some(p) = &pics {
        // Left button, right button and, on a Practice level, the two halves
        // of the ENTER key.
        let enter: &[(usize, f32)] = if state.practice { &[(2, 268.0), (3, 300.0)] } else { &[] };
        for &(i, x) in [(0, 25.0), (1, 522.0)].iter().chain(enter) {
            if let Some(h) = p.prompts.get(i).cloned() {
                add(&mut commands, image_at(h, at(x, PROMPT_Y, 32.0, 32.0)), None);
            }
        }
    }
    text(&mut commands, "Continue", 61.0, LABEL_Y);
    text(&mut commands, "Menu", 561.0, LABEL_Y);
    if state.practice {
        text(&mut commands, "Demo", 342.0, LABEL_Y);
    }
    // Clicking or tapping a prompt picks it; anywhere else continues.
    let buttons: &[(Choice, f32, f32)] = if state.practice { &[(Choice::Preview, 268.0, 150.0), (Choice::Menu, 522.0, 110.0)] } else { &[(Choice::Menu, 522.0, 110.0)] };
    for &(choice, x, w) in buttons {
        let e = commands.spawn((choice, Button, at(x, PROMPT_Y, w, 32.0), Node { position_type: PositionType::Absolute, ..default() })).id();
        commands.entity(canvas).add_child(e);
    }
}
