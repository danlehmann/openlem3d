//! Heads-up display: skill buttons, counters, and assigning skills by
//! clicking or tapping a lemming. Works with mouse, keyboard and touch.

use bevy::prelude::*;
use bevy::text::FontSize;
use l3d_sim::{SUB, Skill};

use crate::Game;
use crate::lemming_cam::LemmingCam;
use crate::menu::AppState;
use crate::scene_render::SceneCamera;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedSkill>()
            .init_resource::<PendingTurner>()
            .add_systems(Startup, (spawn_hud, spawn_turner_marker))
            .add_systems(
                Update,
                (skill_buttons, skill_keys, controls, update_labels, animate_skill_icons, result_panel, bomber_countdown, assign_on_pointer)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, (show_in_play, place_turner_marker));
    }
}

/// The `LEMM.MHC` cells animated on each skill button: the front-view block
/// of the skill's action (first cell, frame count; `docs/spec/lemmings.md`),
/// in [`Skill::ALL`] order. The selected skill animates; the others show
/// their first frame, as in the original.
const SKILL_ICON_FRAMES: [(usize, usize); 9] =
    [(86, 7), (30, 7), (271, 5), (344, 5), (201, 8), (241, 6), (161, 8), (538, 5), (296, 5)];
/// Skill icon animation speed: one frame per simulation tick.
const ICON_FPS: f32 = l3d_sim::TICKS_PER_SECOND as f32;

/// Cuts the skill-button icons from the user's `LEMM.MHC`. Returns `None`
/// when the file can't be read; the buttons then show text only.
fn skill_icons(data: &mut crate::Data, images: &mut Assets<Image>) -> Option<(Vec<Vec<Handle<Image>>>, Handle<Image>)> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let pal = data.0.palette("GFX/LM3D.PAL").ok()?;
    let raw = data.0.read("LEMM/LEMM.MHC").ok()?;
    let mhc = l3d_formats::mhc::MhcFile::parse(&raw, 64).ok()?;
    let mut cell_image = |i: usize| -> Option<Handle<Image>> {
            let cell = mhc.cell(i).ok()?;
            let rgba: Vec<u8> = cell
                .pixels
                .iter()
                .flat_map(|&p| {
                    let [r, g, b] = pal[p as usize];
                    [r, g, b, if p == 0 { 0 } else { 255 }]
                })
                .collect();
            let mut img = Image::new(
                Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
                TextureDimension::D2,
                rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            img.sampler = ImageSampler::nearest();
            Some(images.add(img))
    };
    let skills = SKILL_ICON_FRAMES.iter().map(|&(first, n)| (first..first + n).map(&mut cell_image).collect()).collect::<Option<_>>()?;
    Some((skills, cell_image(LEMMING_CAM_ICON)?))
}

/// The lemming-cam button shows a walker from behind (`LEMM.MHC` cell 24),
/// the view it gives.
const LEMMING_CAM_ICON: usize = 24;

/// Shows the HUD only while a level is being played.
fn show_in_play(state: Res<State<AppState>>, mut roots: Query<&mut Visibility, With<HudRoot>>) {
    if !state.is_changed() {
        return;
    }
    let v = if *state.get() == AppState::Playing { Visibility::Inherited } else { Visibility::Hidden };
    for mut vis in &mut roots {
        *vis = v;
    }
}

/// The skill the next click or tap assigns.
#[derive(Resource, Default)]
pub struct SelectedSkill(pub Option<Skill>);

#[derive(Component)]
struct SkillButton(Skill);

#[derive(Component)]
struct SkillLabel(Skill);

#[derive(Component)]
struct StatusText;

/// Marks the top-level HUD nodes, hidden outside play.
#[derive(Component)]
struct HudRoot;

const BUTTON_IDLE: Color = Color::srgba(0.1, 0.1, 0.2, 0.75);
const BUTTON_SELECTED: Color = Color::srgba(0.8, 0.2, 0.1, 0.9);

fn spawn_hud(mut commands: Commands, mut data: ResMut<crate::Data>, mut images: ResMut<Assets<Image>>) {
    let (icons, cam_icon) = skill_icons(&mut data, &mut images).unzip();
    if let Some(frames) = &icons {
        commands.insert_resource(SkillIconFrames(frames.clone()));
    }
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        Node { position_type: PositionType::Absolute, top: px(8), left: px(8), ..default() },
        StatusText,
        HudRoot,
    ));
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                left: px(0),
                right: px(0),
                justify_content: JustifyContent::Center,
                column_gap: px(4),
                padding: UiRect::all(px(4)),
                ..default()
            },
        ))
        .with_children(|bar| {
            for (label, action) in [("-", Control::Slower), ("+", Control::Faster)] {
                bar.spawn((
                    Button,
                    action,
                    Node { width: px(44), height: px(52), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_child((Text::new(label), TextFont { font_size: FontSize::Px(22.0), ..default() }));
            }
            bar.spawn((
                Button,
                Control::LemmingCam,
                Node { width: px(52), height: px(52), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(BUTTON_IDLE),
            ))
            .with_children(|b| match &cam_icon {
                Some(img) => {
                    b.spawn((ImageNode::new(img.clone()), Node { width: px(44), height: px(44), ..default() }));
                }
                None => {
                    b.spawn((Text::new("Cam"), TextFont { font_size: FontSize::Px(14.0), ..default() }));
                }
            });
            for skill in Skill::ALL {
                bar.spawn((
                    Button,
                    SkillButton(skill),
                    Node {
                        width: px(84),
                        height: px(84),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_children(|b| {
                    if let Some(icons) = &icons {
                        b.spawn((
                            SkillIcon(skill),
                            ImageNode::new(icons[skill as usize][0].clone()),
                            Node { width: px(48), height: px(48), ..default() },
                        ));
                    }
                    b.spawn((
                        Text::new(skill.name()),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextLayout::justify(Justify::Center),
                        SkillLabel(skill),
                    ));
                });
            }
            bar.spawn((
                Button,
                Control::Nuke,
                Node { width: px(64), height: px(52), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(Color::srgba(0.45, 0.05, 0.05, 0.85)),
            ))
            .with_child((Text::new("Nuke"), TextFont { font_size: FontSize::Px(14.0), ..default() }));
        });
    commands.spawn((
        ResultPanel,
        Node {
            position_type: PositionType::Absolute,
            top: percent(30),
            left: percent(25),
            width: percent(50),
            padding: UiRect::all(px(16)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.12, 0.9)),
        Visibility::Hidden,
        children![(
            Text::new(""),
            TextFont { font_size: FontSize::Px(22.0), ..default() },
            TextLayout::justify(Justify::Center),
            ResultText,
        )],
    ));
}

/// Release-rate and nuke buttons.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Control {
    Slower,
    Faster,
    Nuke,
    LemmingCam,
}

/// The end-of-level panel and its text.
#[derive(Component)]
struct ResultPanel;
#[derive(Component)]
struct ResultText;

/// Applies a control: from its button or its key (− / + for the release
/// rate, Alt+Q to nuke as in the original).
/// The image node of a skill button's icon.
#[derive(Component)]
struct SkillIcon(Skill);

/// Animates the selected skill's icon; the others show their first frame.
fn animate_skill_icons(
    selected: Res<SelectedSkill>,
    time: Res<Time>,
    frames: Option<Res<SkillIconFrames>>,
    mut icons: Query<(&SkillIcon, &mut ImageNode)>,
) {
    let Some(frames) = frames else { return };
    let step = (time.elapsed_secs() * ICON_FPS) as usize;
    for (icon, mut node) in &mut icons {
        let list = &frames.0[icon.0 as usize];
        let frame = if selected.0 == Some(icon.0) { step % list.len() } else { 0 };
        if node.image != list[frame] {
            node.image = list[frame].clone();
        }
    }
}

/// The animation frames of each skill button's icon, in [`Skill::ALL`] order.
#[derive(Resource)]
struct SkillIconFrames(Vec<Vec<Handle<Image>>>);

/// Holding − or + (button or key) repeats after this delay, at this rate.
const REPEAT_DELAY: f32 = 0.35;
const REPEAT_PER_SECOND: f32 = 20.0;

/// Applies a control: from its button or its key (− / + for the release
/// rate, Alt+Q to nuke as in the original). − and + repeat while held.
#[allow(clippy::too_many_arguments)]
fn controls(
    buttons: Query<(&Interaction, &Control)>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut held: Local<Option<(Control, f32)>>,
    mut cam_held: Local<bool>,
    mut views: Query<&mut crate::ViewCamera>,
    mut lemming_cam: ResMut<LemmingCam>,
    mut game: ResMut<Game>,
) {
    let pressed = |c: Control| buttons.iter().any(|(i, b)| *i == Interaction::Pressed && *b == c);
    let slower = pressed(Control::Slower) || keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract);
    let faster = pressed(Control::Faster) || keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd);
    let current = if slower { Some(Control::Slower) } else if faster { Some(Control::Faster) } else { None };
    let Some(sim) = &mut game.sim else { return };
    let step = |sim: &mut l3d_sim::Simulation, c: Control| match c {
        Control::Slower => sim.adjust_release_rate(-1),
        Control::Faster => sim.adjust_release_rate(1),
        Control::Nuke | Control::LemmingCam => {}
    };
    match (current, *held) {
        (Some(c), Some((h, since))) if c == h => {
            // Still held: repeat once the delay has passed.
            let t = since + time.delta_secs();
            let before = ((since - REPEAT_DELAY).max(0.0) * REPEAT_PER_SECOND) as u32;
            let after = ((t - REPEAT_DELAY).max(0.0) * REPEAT_PER_SECOND) as u32;
            for _ in before..after {
                step(sim, c);
            }
            *held = Some((c, t));
        }
        (Some(c), _) => {
            step(sim, c);
            *held = Some((c, 0.0));
        }
        (None, _) => *held = None,
    }
    if let Ok(mut view) = views.single_mut()
        && buttons.iter().any(|(i, b)| *i == Interaction::Pressed && *b == Control::LemmingCam)
        && !*cam_held
    {
        lemming_cam.toggle(&mut view);
    }
    *cam_held = buttons.iter().any(|(i, b)| *i == Interaction::Pressed && *b == Control::LemmingCam);
    let nuke_button = buttons.iter().any(|(i, b)| *i == Interaction::Pressed && *b == Control::Nuke);
    if nuke_button || (keys.pressed(KeyCode::AltLeft) && keys.just_pressed(KeyCode::KeyQ)) {
        sim.nuke();
    }
}

/// A countdown digit floating over a bomber.
#[derive(Component)]
struct CountdownDigit;

/// Draws a bomber's countdown (5…1, one digit per 8 ticks) above its head,
/// as the original does.
fn bomber_countdown(
    mut commands: Commands,
    game: Res<Game>,
    camera: Res<SceneCamera>,
    windows: Query<&Window>,
    mut digits: Query<(Entity, &mut Text, &mut Node), With<CountdownDigit>>,
) {
    let (Some(sim), Ok(window)) = (&game.sim, windows.single()) else { return };
    let size = Vec2::new(window.width(), window.height());
    let mut wanted: Vec<(Vec2, u32)> = Vec::new();
    for l in sim.lemmings.iter().filter(|l| !l.gone && !l.state.is_terminal()) {
        let Some(fuse) = l.fuse else { continue };
        let elapsed = l3d_sim::FUSE_TICKS - fuse;
        let digit = 5u32.saturating_sub(elapsed / l3d_sim::FUSE_DIGIT_TICKS);
        if digit == 0 {
            continue;
        }
        let head = Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32)) + Vec3::Y * 0.6;
        let clip = camera.view_proj * head.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        wanted.push((Vec2::new((ndc.x + 1.0) / 2.0 * size.x, (1.0 - ndc.y) / 2.0 * size.y), digit));
    }
    let mut existing = digits.iter_mut();
    for (pos, digit) in &wanted {
        let (left, top) = (px(pos.x - 6.0), px(pos.y - 12.0));
        match existing.next() {
            Some((_, mut text, mut node)) => {
                text.0 = digit.to_string();
                node.left = left;
                node.top = top;
            }
            None => {
                commands.spawn((
                    CountdownDigit,
                    Text::new(digit.to_string()),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    Node { position_type: PositionType::Absolute, left, top, ..default() },
                ));
            }
        }
    }
    for (e, ..) in existing {
        commands.entity(e).despawn();
    }
}

/// Shows the result panel once the level is over.
fn result_panel(
    game: Res<Game>,
    mut panel: Query<&mut Visibility, With<ResultPanel>>,
    mut text: Query<&mut Text, With<ResultText>>,
) {
    let Some(sim) = &game.sim else { return };
    let done = sim.finished();
    for mut v in &mut panel {
        *v = if done { Visibility::Inherited } else { Visibility::Hidden };
    }
    if done {
        let saved = sim.counts.saved;
        let verdict = if saved >= game.save_requirement { "Level complete!" } else { "Not enough lemmings saved." };
        for mut t in &mut text {
            t.0 = format!("{verdict}\nSaved {saved}, needed {}\n\nEsc: choose a level", game.save_requirement);
        }
    }
}

fn skill_buttons(
    buttons: Query<(&Interaction, &SkillButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedSkill>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = Some(button.0);
        }
    }
}

/// F1–F9 select skills in panel order (keyboard mapping of the original
/// unverified).
fn skill_keys(keys: Res<ButtonInput<KeyCode>>, mut selected: ResMut<SelectedSkill>) {
    const KEYS: [KeyCode; 9] = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
    ];
    for (k, skill) in KEYS.iter().zip(Skill::ALL) {
        if keys.just_pressed(*k) {
            selected.0 = Some(skill);
        }
    }
}

fn update_labels(
    game: Res<Game>,
    selected: Res<SelectedSkill>,
    lemming_cam: Res<LemmingCam>,
    mut controls: Query<(&Control, &mut BackgroundColor), Without<SkillButton>>,
    mut labels: Query<(&SkillLabel, &mut Text), Without<StatusText>>,
    mut buttons: Query<(&SkillButton, &mut BackgroundColor)>,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    let Some(sim) = &game.sim else { return };
    for (label, mut text) in &mut labels {
        text.0 = format!("{}\n{}", label.0.name(), sim.skills_left[label.0 as usize]);
    }
    for (button, mut bg) in &mut buttons {
        bg.0 = if selected.0 == Some(button.0) { BUTTON_SELECTED } else { BUTTON_IDLE };
    }
    for (control, mut bg) in &mut controls {
        if *control == Control::LemmingCam {
            bg.0 = if *lemming_cam == LemmingCam::Off { BUTTON_IDLE } else { BUTTON_SELECTED };
        }
    }
    for mut text in &mut status {
        let c = sim.counts;
        let secs = sim.time_left / l3d_sim::TICKS_PER_SECOND;
        text.0 = format!(
            "Out {}   In {} / {}   Released {} / {}   Rate {}   Time {}:{:02}{}",
            sim.out(),
            c.saved,
            game.save_requirement,
            c.released,
            sim.to_release,
            sim.release_rate,
            secs / 60,
            secs % 60,
            if game.paused { "   PAUSED" } else { "" }
        );
    }
}

/// Screen-space radius, in logical pixels per unit of distance-scaled size,
/// within which a click selects a lemming.
const PICK_RADIUS_UNITS: f32 = 0.35;

/// Where a world position appears on screen, in logical pixels, and the
/// pixels per world unit at its depth; `None` behind the camera.
fn to_screen(camera: &SceneCamera, size: Vec2, pos: Vec3) -> Option<(Vec2, f32)> {
    let clip = camera.view_proj * pos.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    let screen = Vec2::new((ndc.x + 1.0) / 2.0 * size.x, (1.0 - ndc.y) / 2.0 * size.y);
    // From the projection's y scale.
    let px_per_unit = camera.view_proj.y_axis.y.abs() / clip.w * size.y / 2.0;
    Some((screen, px_per_unit))
}

/// A lemming's centre in world units.
fn lemming_centre(l: &l3d_sim::Lemming) -> Vec3 {
    Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32)) + Vec3::Y * 0.2
}

/// The lemming a first turner click picked; the next click picks the side it
/// points to, as in the original.
#[derive(Resource, Default)]
pub struct PendingTurner(pub Option<usize>);

/// Assigns the selected skill to the lemming nearest a click or tap. A
/// turner takes two: one on the lemming, then one to the side it should
/// point to.
#[allow(clippy::too_many_arguments)]
fn assign_on_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    ui: Query<&Interaction>,
    camera: Res<SceneCamera>,
    selected: Res<SelectedSkill>,
    mut pending: ResMut<PendingTurner>,
    mut lemming_cam: ResMut<LemmingCam>,
    mut game: ResMut<Game>,
) {
    let Some(sim) = &mut game.sim else { return };
    let picking = lemming_cam.picking();
    if selected.0 != Some(Skill::Turner) || mouse.just_pressed(MouseButton::Right) {
        pending.0 = None;
    }
    if pending.0.is_some_and(|i| !sim.can_assign(i, Skill::Turner)) {
        pending.0 = None;
    }
    let Ok(window) = windows.single() else { return };
    // Clicks on HUD buttons are not world clicks.
    if ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let point = if mouse.just_pressed(MouseButton::Left) {
        window.cursor_position()
    } else {
        // A tap is a touch released without having been dragged.
        touches
            .iter_just_released()
            .find(|t| t.distance().length() < crate::touch::TAP_SLOP)
            .map(|t| t.position())
    };
    let Some(point) = point else { return };
    let size = Vec2::new(window.width(), window.height());
    if let Some(i) = pending.0.take().filter(|_| !picking) {
        // Point to whichever side of the lemming, as seen on screen, the
        // click landed.
        let l = &sim.lemmings[i];
        let centre = lemming_centre(l);
        let Some((at, _)) = to_screen(&camera, size, centre) else { return };
        let side = |d: l3d_sim::Dir| {
            let [x, _, z] = d.delta();
            let tip = to_screen(&camera, size, centre + Vec3::new(x as f32, 0.0, z as f32) * 0.5);
            tip.map_or(f32::MIN, |(tip, _)| (tip - at).normalize_or_zero().dot((point - at).normalize_or_zero()))
        };
        let (acw, cw) = (l.dir.anticlockwise(), l.dir.clockwise());
        sim.assign_turner(i, if side(acw) >= side(cw) { acw } else { cw });
        return;
    }
    let mut best: Option<(usize, f32)> = None;
    for (i, l) in sim.lemmings.iter().enumerate() {
        if l.gone {
            continue;
        }
        let Some((screen, px_per_unit)) = to_screen(&camera, size, lemming_centre(l)) else { continue };
        let d = screen.distance(point);
        if d < PICK_RADIUS_UNITS * px_per_unit.max(20.0) && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    if picking {
        if let Some((i, _)) = best {
            lemming_cam.pick(i);
        }
        return;
    }
    let Some(skill) = selected.0 else { return };
    if let Some((i, _)) = best {
        if skill == Skill::Turner {
            if sim.can_assign(i, skill) {
                pending.0 = Some(i);
            }
        } else {
            sim.assign(i, skill);
        }
    }
}

/// The double arrow over a lemming waiting for its turner direction.
#[derive(Component)]
struct TurnerMarker;

fn spawn_turner_marker(mut commands: Commands) {
    commands.spawn((
        Text::new("<  >"),
        TextFont { font_size: FontSize::Px(20.0), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, ..default() },
        Visibility::Hidden,
        TurnerMarker,
    ));
}

fn place_turner_marker(
    state: Res<State<AppState>>,
    pending: Res<PendingTurner>,
    game: Res<Game>,
    camera: Res<SceneCamera>,
    windows: Query<&Window>,
    mut marker: Query<(&mut Node, &mut Visibility, &ComputedNode), With<TurnerMarker>>,
) {
    let Ok((mut node, mut vis, computed)) = marker.single_mut() else { return };
    let at = (|| {
        if *state.get() != AppState::Playing {
            return None;
        }
        let l = game.sim.as_ref()?.lemmings.get(pending.0?)?;
        let window = windows.single().ok()?;
        let size = Vec2::new(window.width(), window.height());
        to_screen(&camera, size, lemming_centre(l) + Vec3::Y * 0.45).map(|(p, _)| p)
    })();
    match at {
        Some(p) => {
            let half = computed.size() * computed.inverse_scale_factor() / 2.0;
            node.left = px(p.x - half.x);
            node.top = px(p.y - half.y);
            *vis = Visibility::Inherited;
        }
        None => *vis = Visibility::Hidden,
    }
}