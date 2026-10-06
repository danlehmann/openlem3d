//! Assigning skills by clicking or tapping a lemming, the turner's second
//! click, bomber countdowns and the end-of-level result. The skill panel
//! itself is in `panel`. Works with mouse, keyboard and touch.

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
                (skill_keys, nuke_key, result_panel, bomber_countdown, assign_on_pointer)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, (show_in_play, place_turner_marker));
    }
}

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

/// Marks the top-level HUD nodes, hidden outside play.
#[derive(Component)]
struct HudRoot;

/// The end-of-level panel and its text.
#[derive(Component)]
struct ResultPanel;
#[derive(Component)]
struct ResultText;

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

/// Alt+Q nukes, as in the original.
fn nuke_key(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>) {
    if (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)) && keys.just_pressed(KeyCode::KeyQ)
        && let Some(sim) = &mut game.sim
    {
        sim.nuke();
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        ResultPanel,
        HudRoot,
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