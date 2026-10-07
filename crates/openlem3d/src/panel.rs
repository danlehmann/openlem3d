//! The in-level panel, laid out like the original's (`docs/spec/ui-graphics.md`,
//! "In-level panel"): sprites from `ICONS.RNC` and `MINILEMM.GFX` drawn over
//! the 3D view at the original's 320×200 positions, scaled to the window
//! height. On wider windows the right-hand column keeps to the right edge and
//! the skill row to the bottom left.

use bevy::prelude::*;
use l3d_formats::gamedata::Palette;
use l3d_formats::font::Font;
use l3d_formats::icons::{Icons, panel as icon};
use l3d_formats::image::IndexedImage;
use l3d_formats::sheets;
use l3d_sim::Skill;

use crate::hud::SelectedSkill;
use crate::lemming_cam::LemmingCam;
use crate::menu::AppState;
use crate::{Data, Game, LevelInfo, PresetIndex, ViewCamera};

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_art, spawn_panel).chain())
            .add_systems(
                Update,
                (layout, buttons, held_buttons, skill_keys_select, animate, glyph_texts, caption)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, (show_in_play, show_minimap))
            .add_systems(OnEnter(AppState::Playing), select_first_skill.run_if(crate::fresh_level));
    }
}

/// The original's screen size, in which all positions are given.
const SCREEN: Vec2 = Vec2::new(320.0, 200.0);

/// Skill buttons: x position and `MINILEMM` cells (rest cell first), in
/// [`Skill::ALL`] order. Cell ranges after the first three are estimated.
pub(crate) const SKILL_BUTTONS: [(f32, &[usize]); 9] = [
    (45.0, &[19, 20, 21, 22, 23, 24, 25, 24, 23, 22, 21, 20]),
    (72.0, &[26, 27, 28, 29, 30, 31, 32, 31, 30, 29, 28, 27]),
    (98.0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
    (128.0, &[46, 47, 48, 49, 50, 51]),
    (158.0, &[33, 34, 35, 36, 37, 38, 39, 40]),
    (189.0, &[52, 53, 54, 55, 56, 57]),
    (216.0, &[58, 59, 60, 61, 62, 63]),
    (247.0, &[11, 12, 13, 14, 15, 16, 17, 18]),
    (276.0, &[41, 42, 43, 44, 45]),
];
const SKILL_Y: f32 = 168.0;
/// Selected-skill animation: about one cell per 70 Hz frame/4 (a 12-cell
/// ping-pong took about 200 ms; estimated).
const SKILL_FPS: f32 = 50.0;
/// Pause paws: one cell per 70 Hz frame.
const PAWS_FPS: f32 = 70.0;
const ARROW_FPS: f32 = 20.0;

/// Panel graphics, uploaded once.
#[derive(Resource)]
struct Art {
    panel: Vec<Handle<Image>>,
    labels: Vec<Handle<Image>>,
    umbrella: Handle<Image>,
    skills: Vec<Handle<Image>>,
    small: Glyphs,
    large: Glyphs,
}

/// A font's glyph images by character code, and its advance per character.
struct Glyphs {
    first: u8,
    images: Vec<Handle<Image>>,
    size: Vec2,
    digit_advance: f32,
    colon_advance: f32,
}

impl Glyphs {
    fn get(&self, c: char) -> Option<Handle<Image>> {
        let i = (c as usize).checked_sub(self.first as usize)?;
        self.images.get(i).cloned()
    }

    fn advance(&self, c: char) -> f32 {
        if c == ':' { self.colon_advance } else { self.digit_advance }
    }
}

fn to_image(img: &IndexedImage, pal: &Palette) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut image = Image::new(
        Extent3d { width: img.width as u32, height: img.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.to_rgba(pal, true),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn load_art(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else { return };
    let Some(icons) = d.read("GFX/ICONS.RNC").ok().and_then(|raw| Icons::parse(&raw).ok()) else { return };
    let Some(skills) = d.read(sheets::MINILEMM.path).ok().and_then(|raw| sheets::MINILEMM.cut(&raw).ok()) else { return };
    let mut add = |img: &IndexedImage| images.add(to_image(img, &pal));
    let glyphs = |f: &Font, digit_advance: f32, colon_advance: f32, add: &mut dyn FnMut(&IndexedImage) -> Handle<Image>| Glyphs {
        first: f.first,
        images: f.glyphs.iter().map(&mut *add).collect(),
        size: Vec2::new(f.glyph_width as f32, f.glyph_height as f32),
        digit_advance,
        colon_advance,
    };
    let small = glyphs(&icons.small_font, 5.0, 3.0, &mut add);
    let large = glyphs(&icons.large_font, 7.0, 4.0, &mut add);
    commands.insert_resource(Art {
        panel: icons.panel.iter().map(&mut add).collect(),
        labels: icons.labels.iter().map(&mut add).collect(),
        umbrella: add(&icons.umbrella),
        skills: skills.iter().map(&mut add).collect(),
        small,
        large,
    });
}

/// Where an element sits: its top-left corner in the original's 320×200
/// screen, and its size there.
#[derive(Component, Clone, Copy)]
struct PanelPos {
    at: Vec2,
    size: Vec2,
}

/// What clicking an element does.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Skill(Skill),
    Nuke,
    FastForward,
    Camera,
    Pause,
    TurnClockwise,
    TurnAnticlockwise,
    Slower,
    Faster,
    /// The arrow left of the face: leaves the lemming view.
    Arrow,
    LemmingCam,
}

/// A right-aligned string drawn with a panel font; `right` is the x of the
/// last glyph's cell.
#[derive(Component)]
struct GlyphText {
    large: bool,
    right: f32,
    y: f32,
    value: Value,
}

/// The value a [`GlyphText`] shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Value {
    In,
    Out,
    Time,
    SkillCount(Skill),
    MinRate,
    Rate,
    CameraNumber,
}

/// One glyph slot of a [`GlyphText`], counted from the right.
#[derive(Component)]
struct GlyphSlot(usize);

#[derive(Component)]
struct PanelRoot;

/// Elements whose picture changes with the game state.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Animated {
    Skill(Skill),
    FastForward,
    Paws,
    TurnClockwise,
    TurnAnticlockwise,
    Slower,
    Faster,
    Arrow,
    Face,
}

fn spawn_panel(mut commands: Commands, art: Option<Res<Art>>, mut images: ResMut<Assets<Image>>) {
    let Some(art) = art else { return };
    let root = commands
        .spawn((PanelRoot, Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE))
        .id();
    let item = |commands: &mut Commands, at: Vec2, size: Vec2, image: Handle<Image>, action: Option<Action>, anim: Option<Animated>| {
        let mut e = commands.spawn((PanelPos { at, size }, ImageNode::new(image), Node { position_type: PositionType::Absolute, ..default() }));
        if let Some(a) = action {
            e.insert((Button, a));
        }
        if let Some(a) = anim {
            e.insert(a);
        }
        let id = e.id();
        commands.entity(root).add_child(id);
        id
    };
    let icon24 = Vec2::splat(24.0);
    let p = |i: usize| art.panel[i].clone();
    // The minimap and its frame (the map image arrives with the level).
    let frame = images.add(crate::minimap::frame_image());
    let frame = item(&mut commands, Vec2::ZERO, Vec2::splat(70.0), frame, None, None);
    commands.entity(frame).insert(MinimapFrame);
    let map = commands
        .spawn((
            PanelPos { at: Vec2::splat(3.0), size: Vec2::splat(64.0) },
            ImageNode::default(),
            Node { position_type: PositionType::Absolute, ..default() },
            Button,
            crate::minimap::MinimapView,
            Visibility::Hidden,
        ))
        .id();
    commands.entity(root).add_child(map);
    // Right-hand column.
    item(&mut commands, Vec2::new(260.0, 0.0), Vec2::new(32.0, 16.0), art.labels[0].clone(), None, None);
    item(&mut commands, Vec2::new(260.0, 16.0), Vec2::new(32.0, 16.0), art.labels[1].clone(), None, None);
    item(&mut commands, Vec2::new(260.0, 32.0), Vec2::new(32.0, 16.0), art.labels[2].clone(), None, None);
    item(&mut commands, Vec2::new(260.0, 48.0), icon24, p(icon::BOMB), Some(Action::Nuke), None);
    item(&mut commands, Vec2::new(284.0, 48.0), icon24, p(icon::PLAY), Some(Action::FastForward), Some(Animated::FastForward));
    item(&mut commands, Vec2::new(260.0, 72.0), icon24, p(icon::CAMERA), Some(Action::Camera), None);
    item(&mut commands, Vec2::new(284.0, 72.0), icon24, p(icon::PAUSE.start), Some(Action::Pause), Some(Animated::Paws));
    item(&mut commands, Vec2::new(260.0, 96.0), icon24, p(icon::TURN_CLOCKWISE + 1), Some(Action::TurnClockwise), Some(Animated::TurnClockwise));
    item(&mut commands, Vec2::new(284.0, 96.0), icon24, p(icon::TURN_ANTICLOCKWISE + 1), Some(Action::TurnAnticlockwise), Some(Animated::TurnAnticlockwise));
    item(&mut commands, Vec2::new(260.0, 120.0), icon24, p(icon::MINUS + 1), Some(Action::Slower), Some(Animated::Slower));
    item(&mut commands, Vec2::new(284.0, 120.0), icon24, p(icon::PLUS + 1), Some(Action::Faster), Some(Animated::Faster));
    item(&mut commands, Vec2::new(274.0, 143.0), Vec2::splat(32.0), art.umbrella.clone(), None, None);
    // Bottom row.
    item(&mut commands, Vec2::new(0.0, 172.0), icon24, p(icon::RED_DOWN), Some(Action::Arrow), Some(Animated::Arrow));
    item(&mut commands, Vec2::new(24.0, 172.0), icon24, p(icon::FACE + 1), Some(Action::LemmingCam), Some(Animated::Face));
    for (skill, (x, cells)) in Skill::ALL.into_iter().zip(SKILL_BUTTONS) {
        item(&mut commands, Vec2::new(x, SKILL_Y), Vec2::splat(32.0), art.skills[cells[0]].clone(), Some(Action::Skill(skill)), Some(Animated::Skill(skill)));
    }
    // Numbers.
    let text = |commands: &mut Commands, large: bool, right: f32, y: f32, value: Value, slots: usize| {
        let glyphs = if large { &art.large } else { &art.small };
        let t = commands.spawn((GlyphText { large, right, y, value }, Node::default())).id();
        commands.entity(root).add_child(t);
        for i in 0..slots {
            let at = Vec2::new(right - glyphs.digit_advance * i as f32, y);
            let s = commands.spawn((GlyphSlot(i), PanelPos { at, size: glyphs.size }, ImageNode::default(), Node { position_type: PositionType::Absolute, ..default() }, Visibility::Hidden)).id();
            commands.entity(t).add_child(s);
        }
    };
    text(&mut commands, true, 299.0, 2.0, Value::In, 3);
    text(&mut commands, true, 299.0, 18.0, Value::Out, 3);
    text(&mut commands, true, 299.0, 34.0, Value::Time, 4);
    text(&mut commands, false, 272.0, 140.0, Value::MinRate, 2);
    text(&mut commands, false, 296.0, 140.0, Value::Rate, 2);
    text(&mut commands, false, 269.0, 80.0, Value::CameraNumber, 1);
    for (skill, (x, _)) in Skill::ALL.into_iter().zip(SKILL_BUTTONS) {
        text(&mut commands, false, x + 17.0, 188.0, Value::SkillCount(skill), 2);
    }
    let c = commands.spawn((Caption::default(), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(root).add_child(c);
}

/// Window pixels per original pixel.
fn scale(window: &Window) -> f32 {
    (window.height() / SCREEN.y).max(1.0)
}

/// Places every element: the skill row keeps to the bottom left, the
/// right-hand column (x ≥ 256) to the top right, the rest to the top left.
fn layout(windows: Query<&Window>, mut items: Query<(&PanelPos, &mut Node)>) {
    let Ok(window) = windows.single() else { return };
    let s = scale(window);
    let (w, h) = (window.width(), window.height());
    for (p, mut node) in &mut items {
        let x = if p.at.y < 160.0 && p.at.x >= 256.0 { w - (SCREEN.x - p.at.x) * s } else { p.at.x * s };
        let y = if p.at.y >= 160.0 { h - (SCREEN.y - p.at.y) * s } else { p.at.y * s };
        node.left = px(x);
        node.top = px(y);
        node.width = px(p.size.x * s);
        node.height = px(p.size.y * s);
    }
}

#[allow(clippy::too_many_arguments)]
fn buttons(
    pressed: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut selected: ResMut<SelectedSkill>,
    mut game: ResMut<Game>,
    mut lemming_cam: ResMut<LemmingCam>,
    mut views: Query<&mut ViewCamera>,
    cameras: Option<Res<LevelInfo>>,
    mut preset: ResMut<PresetIndex>,
    mut sfx: MessageWriter<crate::sfx::Sfx>,
    mut highlight: ResMut<crate::hud::Highlight>,
    mut pending_turner: ResMut<crate::hud::PendingTurner>,
) {
    for (interaction, action) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(sim) = &mut game.sim else { return };
        match *action {
            Action::Skill(s) => {
                // A skill with none left can't be selected (verified).
                if sim.skills_left[s as usize] > 0 {
                    selected.0 = Some(s);
                    // Given straight to the highlighted lemming; a turner then
                    // waits for the click on the side it should point to.
                    if let Some(i) = highlight.lemming {
                        if s == Skill::Turner {
                            if sim.can_assign(i, s) {
                                pending_turner.0 = Some(i);
                            }
                        } else {
                            let ok = sim.assign(i, s);
                            sfx.write(crate::sfx::Sfx(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" }));
                        }
                    }
                }
            }
            Action::Nuke => {
                if !sim.nuked {
                    sfx.write(crate::sfx::Sfx("VOXFX/GEDDON1"));
                }
                sim.nuke();
            }
            Action::FastForward => game.fast_forward = !game.fast_forward,
            Action::Pause => game.paused = !game.paused,
            Action::Camera => {
                if let (Some(cams), Ok(mut view)) = (&cameras, views.single_mut()) {
                    preset.0 = (preset.0 + 1) % 4;
                    view.set_preset(&cams.cameras[preset.0]);
                    *lemming_cam = LemmingCam::Off;
                }
            }
            // The arrow arms highlighting (the next click on a lemming picks
            // it); clicked again, it disarms and drops the highlight.
            Action::Arrow => {
                if highlight.armed || highlight.lemming.is_some() {
                    *highlight = crate::hud::Highlight::default();
                } else {
                    highlight.armed = true;
                }
            }
            Action::LemmingCam => {
                if let Ok(mut view) = views.single_mut() {
                    match highlight.lemming {
                        // Ride along with the highlighted lemming (switching to
                        // it from another); riding with it already, return.
                        Some(i) if lemming_cam.following() != Some(i) => lemming_cam.follow(i, &view),
                        _ => lemming_cam.toggle(&mut view),
                    }
                }
            }
            Action::TurnClockwise | Action::TurnAnticlockwise | Action::Slower | Action::Faster => {}
        }
    }
}

/// Holding − or + changes the release rate by one per 70 Hz frame, with no
/// initial delay (verified); the turn arrows turn the camera while held.
fn held_buttons(
    held: Query<(&Interaction, &Action)>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut carry: Local<f32>,
    mut game: ResMut<Game>,
    lemming_cam: Res<LemmingCam>,
    mut views: Query<&mut ViewCamera>,
) {
    const RATE_STEPS_PER_SECOND: f32 = 70.0;
    const TURN_SPEED: f32 = 1.8;
    let down = |a: Action| held.iter().any(|(i, b)| *i == Interaction::Pressed && *b == a);
    let slower = down(Action::Slower) || keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract);
    let faster = down(Action::Faster) || keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd);
    if let Some(sim) = &mut game.sim {
        let first = keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::Equal);
        if slower != faster {
            *carry += time.delta_secs() * RATE_STEPS_PER_SECOND;
            // A tap changes the rate by at least one.
            let steps = if first { (*carry as i32).max(1) } else { *carry as i32 };
            *carry -= steps as f32;
            sim.adjust_release_rate(if faster { steps } else { -steps });
        } else {
            *carry = 0.0;
        }
    }
    let turn = down(Action::TurnAnticlockwise) as i32 as f32 - down(Action::TurnClockwise) as i32 as f32;
    if turn != 0.0
        && lemming_cam.following().is_none()
        && let Ok(mut view) = views.single_mut()
    {
        view.yaw -= turn * TURN_SPEED * time.delta_secs();
    }
}

/// F1–F9 also only select skills that are left.
fn skill_keys_select(game: Res<Game>, mut selected: ResMut<SelectedSkill>) {
    if let (Some(sim), Some(s)) = (&game.sim, selected.0)
        && sim.skills_left[s as usize] == 0
        && selected.is_changed()
    {
        selected.0 = None;
    }
}

/// At the start of a level the first skill with some left is selected
/// (verified on level 2).
fn select_first_skill(game: Res<Game>, mut selected: ResMut<SelectedSkill>) {
    selected.0 = game.sim.as_ref().and_then(|sim| Skill::ALL.into_iter().find(|s| sim.skills_left[*s as usize] > 0));
}

#[allow(clippy::too_many_arguments)]
fn animate(
    art: Option<Res<Art>>,
    time: Res<Time>,
    game: Res<Game>,
    selected: Res<SelectedSkill>,
    lemming_cam: Res<LemmingCam>,
    held: Query<(&Interaction, &Action)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut items: Query<(&Animated, &mut ImageNode)>,
    highlight: Res<crate::hud::Highlight>,
) {
    let Some(art) = art else { return };
    let t = time.elapsed_secs();
    let down = |a: Action| held.iter().any(|(i, b)| *i == Interaction::Pressed && *b == a);
    for (anim, mut node) in &mut items {
        let image = match *anim {
            Animated::Skill(s) => {
                let cells = SKILL_BUTTONS[s as usize].1;
                // The selected skill animates while any are left.
                let left = game.sim.as_ref().is_some_and(|sim| sim.skills_left[s as usize] > 0);
                let frame = if selected.0 == Some(s) && left { (t * SKILL_FPS / 4.0) as usize % cells.len() } else { 0 };
                art.skills[cells[frame]].clone()
            }
            Animated::FastForward => art.panel[if game.fast_forward { icon::FAST_FORWARD } else { icon::PLAY }].clone(),
            Animated::Paws => {
                let frame = if game.paused { (t * PAWS_FPS) as usize % icon::PAUSE.len() } else { 0 };
                art.panel[icon::PAUSE.start + frame].clone()
            }
            // Green (the lower cell) while held, red otherwise.
            Animated::TurnClockwise => art.panel[icon::TURN_CLOCKWISE + !down(Action::TurnClockwise) as usize].clone(),
            Animated::TurnAnticlockwise => art.panel[icon::TURN_ANTICLOCKWISE + !down(Action::TurnAnticlockwise) as usize].clone(),
            Animated::Slower => {
                let on = down(Action::Slower) || keys.pressed(KeyCode::Minus);
                art.panel[icon::MINUS + !on as usize].clone()
            }
            Animated::Faster => {
                let on = down(Action::Faster) || keys.pressed(KeyCode::Equal);
                art.panel[icon::PLUS + !on as usize].clone()
            }
            // Green, moving, while it waits for a lemming or one is
            // highlighted.
            Animated::Arrow => match highlight.armed || highlight.lemming.is_some() {
                true => art.panel[icon::GREEN_DOWN.start + (t * ARROW_FPS) as usize % icon::GREEN_DOWN.len()].clone(),
                false => art.panel[icon::RED_DOWN].clone(),
            },
            // Squinting when idle, eyes open when armed or riding along.
            Animated::Face => art.panel[icon::FACE + (*lemming_cam == LemmingCam::Off) as usize].clone(),
        };
        if node.image != image {
            node.image = image;
        }
    }
}

/// The string a counter shows.
fn value_string(v: Value, game: &Game, preset: usize) -> String {
    let Some(sim) = &game.sim else { return String::new() };
    match v {
        // Counts down the lemmings still needed, then counts those saved
        // beyond that (observed in the original's demos).
        Value::In => game.save_requirement.abs_diff(sim.counts.saved).to_string(),
        Value::Out => sim.out().to_string(),
        Value::Time => {
            let secs = sim.time_left / l3d_sim::TICKS_PER_SECOND;
            format!("{}:{:02}", secs / 60, secs % 60)
        }
        Value::SkillCount(s) => match sim.skills_left[s as usize] {
            0 => String::new(),
            n => n.to_string(),
        },
        Value::MinRate => sim.min_release_rate.to_string(),
        Value::Rate => sim.release_rate.to_string(),
        Value::CameraNumber => (preset + 1).to_string(),
    }
}

/// Fills each counter's glyph slots, right-aligned.
fn glyph_texts(
    art: Option<Res<Art>>,
    game: Res<Game>,
    preset: Res<PresetIndex>,
    texts: Query<(&GlyphText, &Children)>,
    mut slots: Query<(&GlyphSlot, &mut PanelPos, &mut ImageNode, &mut Visibility)>,
) {
    let Some(art) = art else { return };
    for (text, children) in &texts {
        let glyphs = if text.large { &art.large } else { &art.small };
        let s = value_string(text.value, &game, preset.0);
        // Positions from the right: each character's cell x.
        // Each character's cell starts its own advance left of the next one's
        // ("3:53": digits at 281, 292, 299 and the colon at 288).
        let mut placed: Vec<(f32, char)> = Vec::new();
        for c in s.chars().rev() {
            let x = placed.last().map_or(text.right, |&(next_x, _)| next_x - glyphs.advance(c));
            placed.push((x, c));
        }
        for child in children.iter() {
            let Ok((slot, mut pos, mut node, mut vis)) = slots.get_mut(child) else { continue };
            match placed.get(slot.0).and_then(|&(x, c)| Some((x, glyphs.get(c)?))) {
                Some((x, image)) => {
                    pos.at = Vec2::new(x, text.y);
                    if node.image != image {
                        node.image = image;
                    }
                    *vis = Visibility::Inherited;
                }
                None => *vis = Visibility::Hidden,
            }
        }
    }
}

/// The minimap's frame.
#[derive(Component)]
struct MinimapFrame;

/// Shows the level's minimap image and its frame, unless the level hides
/// its minimap; then neither shows (observed on `LEVEL.019`, `044`, `062`).
fn show_minimap(
    map: Option<Res<crate::minimap::Minimap>>,
    mut views: Query<(&mut ImageNode, &mut Visibility), With<crate::minimap::MinimapView>>,
    mut frames: Query<&mut Visibility, (With<MinimapFrame>, Without<crate::minimap::MinimapView>)>,
) {
    let Some(map) = map else { return };
    let v = if map.shown { Visibility::Inherited } else { Visibility::Hidden };
    for mut vis in &mut frames {
        vis.set_if_neq(v);
    }
    for (mut node, mut vis) in &mut views {
        if node.image != map.image {
            node.image = map.image.clone();
        }
        vis.set_if_neq(v);
    }
}

/// The panel shows while playing, but not during the level preview (an
/// interstitial in the original, without the panel).
fn show_in_play(
    state: Res<State<AppState>>,
    opts: Res<crate::Options>,
    preview: Res<crate::briefing::Preview>,
    mut roots: Query<&mut Visibility, With<PanelRoot>>,
) {
    let v = if *state.get() == AppState::Playing && !opts.no_hud && !preview.active { Visibility::Inherited } else { Visibility::Hidden };
    for mut vis in &mut roots {
        if *vis != v {
            *vis = v;
        }
    }
}

/// The line of large text at the bottom left, above the arrow and the face
/// (position estimated): "Replaying" and "Click to Play" during a replay,
/// otherwise the state of the lemming under the pointer, as in the original
/// ("Walker", "Sliding").
#[derive(Component, Default)]
struct Caption(String);

const CAPTION_AT: Vec2 = Vec2::new(3.0, 158.0);
/// Seconds each replay message shows (estimated).
const REPLAY_BLINK: f32 = 1.5;

/// A lemming's state as the caption names it. "Walker" and "Sliding" are
/// verified; the other names are ours.
fn state_name(l: &l3d_sim::Lemming) -> Option<&'static str> {
    use l3d_sim::State;
    if l.fuse.is_some() && !l.state.is_terminal() {
        return Some("Bomber");
    }
    Some(match l.state {
        State::Walking if l.climber => "Climber",
        State::Walking if l.floater => "Floater",
        State::Walking => "Walker",
        State::Sliding => "Sliding",
        State::Falling { .. } => "Faller",
        State::Floating => "Floater",
        State::Climbing => "Climber",
        State::Blocking => "Blocker",
        State::Turning { .. } => "Turner",
        State::Building { .. } => "Builder",
        State::Bashing => "Basher",
        State::Mining => "Miner",
        State::Digging => "Digger",
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn caption(
    mut commands: Commands,
    art: Option<Res<crate::title::Art>>,
    game: Res<Game>,
    time: Res<Time>,
    camera: Res<crate::scene_render::SceneCamera>,
    windows: Query<&Window>,
    ui: Query<&Interaction>,
    mut captions: Query<(Entity, &mut Caption)>,
) {
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else { return };
    let Ok((entity, mut shown)) = captions.single_mut() else { return };
    let text = if game.replay.as_ref().is_some_and(|r| r.demo.is_some()) {
        "Demo"
    } else if game.replay.is_some() {
        if ((time.elapsed_secs() / REPLAY_BLINK) as u32).is_multiple_of(2) { "Replaying" } else { "Click to Play" }
    } else {
        (|| {
            let window = windows.single().ok()?;
            let point = window.cursor_position()?;
            if ui.iter().any(|i| *i != Interaction::None) {
                return None;
            }
            let sim = game.sim.as_ref()?;
            let i = crate::hud::lemming_at(sim, &camera, Vec2::new(window.width(), window.height()), point)?;
            state_name(&sim.lemmings[i])
        })()
        .unwrap_or("")
    };
    if shown.0 == text {
        return;
    }
    shown.0 = text.to_string();
    commands.entity(entity).despawn_children();
    let (glyphs, _) = font.layout(text);
    for (x, image) in glyphs {
        let g = commands
            .spawn((PanelPos { at: CAPTION_AT + Vec2::X * x, size: font.size }, ImageNode::new(image), Node { position_type: PositionType::Absolute, ..default() }, Pickable::IGNORE))
            .id();
        commands.entity(entity).add_child(g);
    }
}
