//! Assigning skills by clicking or tapping a lemming, the turner's second
//! click. The skill panel
//! itself is in `panel`. Works with mouse, keyboard and touch.

use bevy::prelude::*;
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
            .init_resource::<Highlight>()
            .add_systems(Startup, spawn_turner_marker)
            .add_systems(
                Update,
                (skill_keys, nuke_key, assign_on_pointer, take_over)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, (place_turner_marker, clear_highlight))
            .add_systems(OnEnter(AppState::Briefing), |mut h: ResMut<Highlight>| *h = Highlight::default());
    }
}

/// The skill the next click or tap assigns.
#[derive(Resource, Default)]
pub struct SelectedSkill(pub Option<Skill>);

/// Alt+Q nukes, as in the original.
fn nuke_key(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>) {
    if (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)) && keys.just_pressed(KeyCode::KeyQ)
        && let Some(sim) = &mut game.sim
    {
        sim.nuke();
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

/// During a replay, a click or tap on the view hands control back to the
/// player ("Click to Play"); the click itself does nothing else (skills are
/// only given from the next one).
pub(crate) fn take_over(
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    settings: Res<crate::settings::Settings>,
    mut game: ResMut<Game>,
) {
    let button = settings.action_button();
    let clicked = mouse.just_pressed(button) || touches.iter_just_released().any(|t| t.distance().length() < crate::touch::TAP_SLOP);
    if game.replay.is_none() || !clicked || ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    game.replay = None;
    mouse.clear_just_pressed(button);
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

/// The lemming nearest a screen point (logical pixels), if one is close
/// enough to click.
pub fn lemming_at(sim: &l3d_sim::Simulation, camera: &SceneCamera, size: Vec2, point: Vec2) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, l) in sim.lemmings.iter().enumerate() {
        if l.gone {
            continue;
        }
        let Some((screen, px_per_unit)) = to_screen(camera, size, lemming_centre(l)) else { continue };
        let d = screen.distance(point);
        if d < PICK_RADIUS_UNITS * px_per_unit.max(20.0) && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Highlighting (the panel's arrow; measured in the original): `on` from
/// the arrow's click, which highlights the lemming nearest the middle of
/// the view, until the arrow is clicked again; meanwhile a click on a
/// lemming moves the highlight to it. The highlighted lemming carries an
/// arrow, a skill clicked is given to it, and the face rides along with it.
#[derive(Resource, Default)]
pub struct Highlight {
    pub on: bool,
    pub lemming: Option<usize>,
}

/// The lemming a first turner click picked; the next click picks the side it
/// points to, as in the original.
#[derive(Resource, Default)]
pub struct PendingTurner(pub Option<usize>);

/// Assigns the selected skill to the lemming nearest a click or tap. A
/// turner takes two: one on the lemming, then one to the side it should
/// point to.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assign_on_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    ui: Query<&Interaction>,
    camera: Res<SceneCamera>,
    selected: Res<SelectedSkill>,
    mut pending: ResMut<PendingTurner>,
    mut lemming_cam: ResMut<LemmingCam>,
    mut game: ResMut<Game>,
    mut sfx: MessageWriter<crate::sfx::Sfx>,
    settings: Res<crate::settings::Settings>,
    mut highlight: ResMut<Highlight>,
    views: Query<&crate::ViewCamera>,
) {
    // During a replay a click only takes over (`take_over`, next).
    if game.replay.is_some() {
        return;
    }
    let Some(sim) = &mut game.sim else { return };
    let picking = lemming_cam.picking();
    if selected.0 != Some(Skill::Turner) || mouse.just_pressed(settings.turn_button()) {
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
    let point = if mouse.just_pressed(settings.action_button()) {
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
        let Some(to) = turner_choice(&camera, size, &sim.lemmings[i], point) else { return };
        let ok = sim.assign_turner(i, to);
        sfx.write(crate::sfx::Sfx(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" }));
        return;
    }
    let best = lemming_at(sim, &camera, size, point);
    if highlight.on {
        // A click on a lemming moves the highlight to it; riding along
        // with another, the view switches to it.
        if let Some(i) = best {
            highlight.lemming = Some(i);
            if lemming_cam.following().is_some()
                && let Ok(view) = views.single()
            {
                lemming_cam.follow(i, view);
            }
            sfx.write(crate::sfx::Sfx("SPOTFX/SCALE"));
        }
        return;
    }
    if picking {
        if let Some(i) = best {
            lemming_cam.pick(i);
            // Riding along also highlights it (seen in the original).
            *highlight = Highlight { on: true, lemming: Some(i) };
        }
        return;
    }
    let Some(skill) = selected.0 else { return };
    if let Some(i) = best {
        if skill == Skill::Turner {
            if sim.can_assign(i, skill) {
                pending.0 = Some(i);
            } else {
                sfx.write(crate::sfx::Sfx("VOXFX/UH_UH1"));
            }
        } else {
            let ok = sim.assign(i, skill);
            sfx.write(crate::sfx::Sfx(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" }));
        }
    }
}


/// For a lemming waiting for its turner direction and a screen point: the
/// screen-space unit vectors of its two possible sides (anticlockwise,
/// clockwise), its centre on screen, and how well the point lines up with
/// each side (cosine). `None` if the lemming is behind the camera.
fn turner_sides(camera: &SceneCamera, size: Vec2, l: &l3d_sim::Lemming, point: Option<Vec2>) -> Option<(Vec2, [Vec2; 2], [f32; 2])> {
    let centre = lemming_centre(l);
    let (at, _) = to_screen(camera, size, centre)?;
    let screen = |d: l3d_sim::Dir| {
        let [x, _, z] = d.delta();
        to_screen(camera, size, centre + Vec3::new(x as f32, 0.0, z as f32) * 0.5).map_or(Vec2::ZERO, |(tip, _)| (tip - at).normalize_or_zero())
    };
    let dirs = [screen(l.dir.anticlockwise()), screen(l.dir.clockwise())];
    let towards = point.map_or(Vec2::ZERO, |p| (p - at).normalize_or_zero());
    Some((at, dirs, dirs.map(|d| d.dot(towards))))
}

/// The direction a click at `point` gives a waiting turner: the side, as
/// seen on screen, the click landed on.
fn turner_choice(camera: &SceneCamera, size: Vec2, l: &l3d_sim::Lemming, point: Vec2) -> Option<l3d_sim::Dir> {
    let (_, _, [acw, cw]) = turner_sides(camera, size, l, Some(point))?;
    Some(if acw >= cw { l.dir.anticlockwise() } else { l.dir.clockwise() })
}

/// One of the two arrows beside a lemming waiting for its turner direction
/// (0: anticlockwise side, 1: clockwise), from `MOUSE.RNC`'s white arrows.
#[derive(Component)]
struct TurnerArrow(usize);

/// `MOUSE.RNC` cells 11–42: a white arrow in 32 directions, clockwise from
/// straight up.
#[derive(Resource)]
struct ArrowCells(Vec<Handle<Image>>);
const ARROW_FIRST: usize = 11;
const ARROW_DIRECTIONS: usize = 32;
/// The arrows' size, and their distance from the lemming's centre, in
/// 320×200 pixels.
const ARROW_SIZE: f32 = 16.0;
const ARROW_DISTANCE: f32 = 12.0;
/// While the pointer is not clearly on one side, the two arrows take turns,
/// each shown this long (seconds; observed in the original's Practice
/// "Turner" demo: never both at once, 7–10 swaps a second).
const ARROW_SWAP: f32 = 0.12;
/// How much better one side must line up with the pointer than the other,
/// and how far (in 320×200 pixels) the pointer must be from the lemming,
/// for the choice to count as clear.
const ARROW_CLEAR_MARGIN: f32 = 0.5;
const ARROW_CLEAR_DISTANCE: f32 = 4.0;

fn spawn_turner_marker(mut commands: Commands, mut data: ResMut<crate::Data>, mut images: ResMut<Assets<Image>>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let d = &mut data.0;
    let sheet = &l3d_formats::sheets::MOUSE;
    let (Ok(pal), Ok(raw)) = (d.palette("GFX/LM3D.PAL"), d.read(sheet.path)) else { return };
    let Ok(cells) = sheet.cut(&raw) else { return };
    let handles = cells
        .iter()
        .skip(ARROW_FIRST)
        .take(ARROW_DIRECTIONS)
        .map(|c| {
            let mut image = Image::new(
                Extent3d { width: c.width as u32, height: c.height as u32, depth_or_array_layers: 1 },
                TextureDimension::D2,
                c.to_rgba(&pal, true),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            image.sampler = ImageSampler::nearest();
            images.add(image)
        })
        .collect();
    commands.insert_resource(ArrowCells(handles));
    for side in 0..2 {
        commands.spawn((
            TurnerArrow(side),
            ImageNode::default(),
            Node { position_type: PositionType::Absolute, ..default() },
            Pickable::IGNORE,
            Visibility::Hidden,
        ));
    }
}

/// Shows the arrows beside a lemming waiting for its turner direction: they
/// take turns until the pointer is clearly on one side, then only that side's
/// arrow shows, previewing where the next click points the turner.
#[allow(clippy::too_many_arguments)]
fn place_turner_marker(
    state: Res<State<AppState>>,
    pending: Res<PendingTurner>,
    game: Res<Game>,
    camera: Res<SceneCamera>,
    cells: Option<Res<ArrowCells>>,
    time: Res<Time>,
    windows: Query<&Window>,
    mut arrows: Query<(&TurnerArrow, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    let shown = (|| {
        if *state.get() != AppState::Playing {
            return None;
        }
        let l = game.sim.as_ref()?.lemmings.get(pending.0?)?;
        let window = windows.single().ok()?;
        let size = Vec2::new(window.width(), window.height());
        let pointer = window.cursor_position();
        let (at, dirs, fit) = turner_sides(&camera, size, l, pointer)?;
        let scale = size.y / 200.0;
        let clear = pointer.is_some_and(|p| p.distance(at) > ARROW_CLEAR_DISTANCE * scale) && (fit[0] - fit[1]).abs() > ARROW_CLEAR_MARGIN;
        let first = ((time.elapsed_secs() / ARROW_SWAP) as u32).is_multiple_of(2);
        let visible = if clear { [fit[0] >= fit[1], fit[1] > fit[0]] } else { [first, !first] };
        Some((at, dirs, visible, scale))
    })();
    for (arrow, mut image, mut node, mut vis) in &mut arrows {
        let Some((at, dirs, _, scale)) = shown.filter(|s| s.2[arrow.0]) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let d = dirs[arrow.0];
        // Clockwise from straight up, in the 32 steps of the arrow cells.
        let angle = d.x.atan2(-d.y).rem_euclid(std::f32::consts::TAU);
        let k = (angle / std::f32::consts::TAU * ARROW_DIRECTIONS as f32).round() as usize % ARROW_DIRECTIONS;
        if let Some(h) = cells.as_ref().and_then(|c| c.0.get(k))
            && image.image != *h
        {
            image.image = h.clone();
        }
        let half = ARROW_SIZE * scale / 2.0;
        let centre = at + d * ARROW_DISTANCE * scale;
        node.left = px(centre.x - half);
        node.top = px(centre.y - half);
        node.width = px(half * 2.0);
        node.height = px(half * 2.0);
        vis.set_if_neq(Visibility::Inherited);
    }
}

/// Drops the highlight when its lemming leaves play or a new level starts.
fn clear_highlight(game: Res<Game>, current: Res<crate::CurrentLevel>, mut highlight: ResMut<Highlight>) {
    if current.loaded.is_none() {
        *highlight = Highlight::default();
        return;
    }
    let alive = |i: usize| game.sim.as_ref().and_then(|s| s.lemmings.get(i)).is_some_and(|l| !l.gone);
    // The highlighted lemming gone, highlighting ends (seen when the
    // lemming ridden with died or left).
    if highlight.lemming.is_some_and(|i| !alive(i)) {
        *highlight = Highlight::default();
    }
}

/// The lemming on screen nearest the middle of the view, the one the arrow
/// highlights when switched on (seen in the original: the one nearest the
/// view's centre).
pub fn nearest_to_centre(sim: &l3d_sim::Simulation, camera: &SceneCamera, size: Vec2) -> Option<usize> {
    let mid = size / 2.0;
    sim.lemmings
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.gone)
        .filter_map(|(i, l)| {
            let (p, _) = to_screen(camera, size, lemming_centre(l))?;
            (p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y).then_some((i, p.distance(mid)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}
