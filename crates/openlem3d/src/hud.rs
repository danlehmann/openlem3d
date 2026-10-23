//! Assigning skills by clicking or tapping a lemming, the turner's second
//! click. The skill panel
//! itself is in `panel`. Works with mouse, keyboard and touch.

use l3d_engine::prelude::*;
use l3d_sim::Skill;

use crate::World;
use crate::menu::AppState;
use crate::scene_render::SceneCamera;
use crate::ui::{Node, NodeId, UiNode, px};

/// The skill selected, and how many times it has been set (each time says
/// its name, even the same skill again).
#[derive(Default)]
pub struct SelectedSkill(pub Option<Skill>, pub u64);

impl SelectedSkill {
    pub fn set(&mut self, skill: Option<Skill>) {
        self.0 = skill;
        self.1 += 1;
    }
}

/// Leaving for a briefing ends highlighting.
pub(crate) fn enter_briefing(w: &mut World) {
    w.highlight = Highlight::default();
}

/// Alt+Q nukes, as in the original.
pub(crate) fn nuke_key(w: &mut World) {
    let (keys, game) = (&w.e.input.keys, &mut w.game);
    if (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight))
        && keys.just_pressed(KeyCode::KeyQ)
        && let Some(sim) = &mut game.sim
    {
        sim.nuke();
    }
}

/// F1–F9 select skills in panel order (keyboard mapping of the original
/// unverified).
pub(crate) fn skill_keys(w: &mut World) {
    let (keys, selected) = (&w.e.input.keys, &mut w.selected);
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
            selected.set(Some(skill));
        }
    }
}

/// During a replay, a click or tap on the view hands control back to the
/// player ("Click to Play"); the click itself does nothing else (skills are
/// only given from the next one).
pub(crate) fn take_over(w: &mut World) {
    let (mouse, touches) = (&mut w.e.input.mouse, &w.e.input.touches);
    let (settings, game) = (&w.settings, &mut w.game);
    let button = settings.action_button();
    let clicked = mouse.just_pressed(button)
        || touches
            .iter_just_released()
            .any(|t| t.distance().length() < crate::touch::TAP_SLOP);
    if game.replay.is_none() || !clicked || w.ui.any_active() {
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
    Vec3::from_array(l.pos.map(crate::units)) + Vec3::Y * 0.2
}

/// The lemming nearest a screen point (logical pixels), if one is close
/// enough to click.
pub fn lemming_at(
    sim: &l3d_sim::Simulation,
    camera: &SceneCamera,
    size: Vec2,
    point: Vec2,
) -> Option<usize> {
    lemming_near(sim, camera, size, point, None)
}

/// The lemming nearest a screen point (logical pixels), if one is close
/// enough to click, other than `except`.
pub fn lemming_near(
    sim: &l3d_sim::Simulation,
    camera: &SceneCamera,
    size: Vec2,
    point: Vec2,
    except: Option<usize>,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, l) in sim.lemmings.iter().enumerate() {
        if l.gone || except == Some(i) {
            continue;
        }
        let Some((screen, px_per_unit)) = to_screen(camera, size, lemming_centre(l)) else {
            continue;
        };
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
/// arrow, a skill clicked is given to it without being selected, and the
/// face rides along with it.
#[derive(Default)]
pub struct Highlight {
    pub on: bool,
    pub lemming: Option<usize>,
    /// The last skill given while highlighting; it becomes the selected
    /// skill when highlighting is switched off (owner's observation).
    pub last_skill: Option<Skill>,
}

/// The lemming a first turner click picked; the next click picks the side it
/// points to, as in the original.
#[derive(Default)]
pub struct PendingTurner(pub Option<usize>);

/// Assigns the selected skill to the lemming nearest a click or tap. A
/// turner takes two: one on the lemming, then one to the side it should
/// point to.
pub(crate) fn assign_on_pointer(w: &mut World) {
    let (mouse, touches, starts) = (&w.e.input.mouse, &w.e.input.touches, &w.touch_starts);
    let (window, screen, camera) = (&w.e.window, &w.screen, &w.camera);
    let (selected, pending, lemming_cam) = (&mut w.selected, &mut w.pending, &mut w.lemming_cam);
    let (game, sfx, settings) = (&mut w.game, &mut w.sfx, &w.settings);
    let (highlight, view) = (&mut w.highlight, &w.view);
    // During a replay a click only takes over (`take_over`, next).
    if game.replay.is_some() {
        return;
    }
    // Paused, skills can't be given (owner's observation); selecting,
    // highlighting and riding along still work.
    let paused = game.paused;
    let Some(sim) = &mut game.sim else { return };
    let picking = lemming_cam.picking();
    let following = lemming_cam.following();
    // A turner given to the lemming ridden with, or to the highlighted one,
    // waits for its side without being selected.
    let waiting = pending.0.is_some()
        && (pending.0 == following || (highlight.on && pending.0 == highlight.lemming));
    if (selected.0 != Some(Skill::Turner) && !waiting) || mouse.just_pressed(settings.turn_button())
    {
        pending.0 = None;
    }
    if pending.0.is_some_and(|i| !sim.can_assign(i, Skill::Turner)) {
        pending.0 = None;
    }
    // Clicks on HUD buttons are not world clicks.
    if w.ui.any_active() {
        return;
    }
    let point = if mouse.just_pressed(settings.action_button()) {
        screen.cursor(window)
    } else {
        // A tap is a touch released without having been dragged, and not
        // from a button (a turner given by its button with a lemming
        // highlighted waits for the next tap, not this one).
        touches
            .iter_just_released()
            .find(|t| t.distance().length() < crate::touch::TAP_SLOP && !starts.on_button(t.id()))
            .map(|t| screen.on_screen(t.position()))
    };
    let Some(point) = point else { return };
    let size = screen.size;
    // Riding along, nothing is selected (verified): a turner given to the
    // lemming ridden with points to the side of the screen clicked (left
    // half: its left), and a click on another lemming rides along with it
    // instead.
    if let Some(f) = following {
        if pending.0 == Some(f) {
            if paused {
                return;
            }
            pending.0 = None;
            let head = view.head;
            let sides = lemming_sides(&sim.lemmings[f]);
            let ok = sim.assign_turner(f, sides[riding_right_half(head, size, point) as usize]);
            sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
            return;
        }
        if let Some(i) = lemming_near(sim, camera, size, point, Some(f)) {
            lemming_cam.ride(i, highlight, selected, view);
            sfx.play("SPOTFX/SCALE");
        }
        return;
    }
    if paused && pending.0.is_some() {
        return;
    }
    if let Some(i) = pending.0.take().filter(|_| !picking) {
        let Some(to) = turner_choice(camera, size, &sim.lemmings[i], point) else {
            return;
        };
        let ok = sim.assign_turner(i, to);
        sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
        return;
    }
    let best = lemming_at(sim, camera, size, point);
    // Armed, the face rides along with the lemming clicked, even with a
    // skill selected (verified).
    if picking {
        if let Some(i) = best {
            lemming_cam.ride(i, highlight, selected, view);
        }
        return;
    }
    if highlight.on {
        // A click on a lemming moves the highlight to it.
        if let Some(i) = best {
            highlight.lemming = Some(i);
            sfx.play("SPOTFX/SCALE");
        }
        return;
    }
    let Some(skill) = selected.0.filter(|_| !paused) else {
        return;
    };
    if let Some(i) = best {
        if skill == Skill::Turner {
            if sim.can_assign(i, skill) {
                pending.0 = Some(i);
            } else {
                sfx.play("VOXFX/UH_UH1");
            }
        } else {
            let ok = sim.assign(i, skill);
            sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
        }
    }
}

/// For a lemming waiting for its turner direction and a screen point: the
/// screen-space unit vectors of its two possible sides (anticlockwise,
/// clockwise), its centre on screen, and how well the point lines up with
/// each side (cosine). `None` if the lemming is behind the camera.
fn turner_sides(
    camera: &SceneCamera,
    size: Vec2,
    l: &l3d_sim::Lemming,
    point: Option<Vec2>,
) -> Option<(Vec2, [Vec2; 2], [f32; 2])> {
    let centre = lemming_centre(l);
    let (at, _) = to_screen(camera, size, centre)?;
    let screen = |d: l3d_sim::Dir| {
        let [x, _, z] = d.delta();
        to_screen(
            camera,
            size,
            centre + Vec3::new(x as f32, 0.0, z as f32) * 0.5,
        )
        .map_or(Vec2::ZERO, |(tip, _)| (tip - at).normalize_or_zero())
    };
    let dirs = [screen(l.dir.anticlockwise()), screen(l.dir.clockwise())];
    let towards = point.map_or(Vec2::ZERO, |p| (p - at).normalize_or_zero());
    Some((at, dirs, dirs.map(|d| d.dot(towards))))
}

/// The direction a click at `point` gives a waiting turner: the side, as
/// seen on screen, the click landed on.
fn turner_choice(
    camera: &SceneCamera,
    size: Vec2,
    l: &l3d_sim::Lemming,
    point: Vec2,
) -> Option<l3d_sim::Dir> {
    let (_, _, [acw, cw]) = turner_sides(camera, size, l, Some(point))?;
    Some(if acw >= cw {
        l.dir.anticlockwise()
    } else {
        l.dir.clockwise()
    })
}

/// The two arrows beside a lemming waiting for its turner direction (0:
/// anticlockwise side, 1: clockwise), and `MOUSE.RNC` cells 11–42 for them:
/// a white arrow in 32 directions, clockwise from straight up.
#[derive(Default)]
pub(crate) struct TurnerArrows {
    nodes: [Option<NodeId>; 2],
    cells: Vec<Handle<Image>>,
}
const ARROW_FIRST: usize = 11;
const ARROW_DIRECTIONS: usize = 32;
/// The arrows' size, and their distance from the lemming's centre, in
/// 320×200 pixels.
const ARROW_SIZE: f32 = 16.0;
const ARROW_DISTANCE: f32 = 12.0;
/// Riding along, a turner's arrow is centred this far to the side of the
/// screen's centre, at this height (320×200 pixels; measured).
const RIDING_ARROW_OFFSET: f32 = 16.0;
const RIDING_ARROW_Y: f32 = 85.0;
/// While the pointer is not clearly on one side, the two arrows take turns,
/// each shown this long (seconds; observed in the original's Practice
/// "Turner" demo: never both at once, 7–10 swaps a second).
const ARROW_SWAP: f32 = 0.12;
/// How much better one side must line up with the pointer than the other,
/// and how far (in 320×200 pixels) the pointer must be from the lemming,
/// for the choice to count as clear.
const ARROW_CLEAR_MARGIN: f32 = 0.5;
const ARROW_CLEAR_DISTANCE: f32 = 4.0;

pub(crate) fn spawn_turner_marker(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let sheet = &l3d_formats::sheets::MOUSE;
    let (Ok(pal), Ok(raw)) = (d.palette("GFX/LM3D.PAL"), d.read(sheet.path)) else {
        return;
    };
    let Ok(cells) = sheet.cut(&raw) else { return };
    let handles = cells
        .iter()
        .skip(ARROW_FIRST)
        .take(ARROW_DIRECTIONS)
        .map(|c| images.add(crate::texture::indexed_image(c, &pal, true)))
        .collect();
    w.arrows.cells = handles;
    for side in 0..2 {
        w.arrows.nodes[side] = Some(w.ui.root(UiNode::image(Handle::default()).hidden()));
    }
}

/// Shows the arrows beside a lemming waiting for its turner direction: they
/// take turns until the pointer is clearly on one side, then only that side's
/// arrow shows, previewing where the next click points the turner.
pub(crate) fn place_turner_marker(w: &mut World) {
    let (pending, game, camera, time) = (&w.pending, &w.game, &w.camera, w.e.time.game);
    let (window, screen, lemming_cam) = (&w.e.window, &w.screen, &w.lemming_cam);
    let shown = (|| {
        if w.state != AppState::Playing {
            return None;
        }
        // Riding along with a turner waiting for its side: one arrow near
        // the middle of the view, pointing to the half of the screen the
        // pointer is in, the side a click there picks (verified).
        if let Some(f) = lemming_cam.following() {
            if pending.0 != Some(f) {
                return None;
            }
            // With the head turned to the side, the arrow and the halves
            // turn with the lemming's sides as seen from its eyes (owner's
            // observation: looking a quarter turn right, the arrows point up
            // and down, picked by the top and bottom halves).
            let head = w.view.head;
            let size = screen.size;
            let scale = size.y / 200.0;
            let left = screen
                .cursor(window)
                .is_none_or(|p| !riding_right_half(head, size, p));
            let (right, down) = riding_axes(head);
            let side = if left { -1.0 } else { 1.0 };
            let offset = Vec2::new(
                side * (RIDING_ARROW_OFFSET - ARROW_DISTANCE),
                RIDING_ARROW_Y - 100.0,
            ) * scale;
            let at = size / 2.0 + right * offset.x + down * offset.y;
            return Some((at, [-right, right], [left, !left], scale));
        }
        let l = game.sim.as_ref()?.lemmings.get(pending.0?)?;
        let size = screen.size;
        let pointer = screen.cursor(window);
        let (at, dirs, fit) = turner_sides(camera, size, l, pointer)?;
        let scale = size.y / 200.0;
        let clear = pointer.is_some_and(|p| p.distance(at) > ARROW_CLEAR_DISTANCE * scale)
            && (fit[0] - fit[1]).abs() > ARROW_CLEAR_MARGIN;
        let first = ((time.elapsed_secs() / ARROW_SWAP) as u32).is_multiple_of(2);
        let visible = if clear {
            [fit[0] >= fit[1], fit[1] > fit[0]]
        } else {
            [first, !first]
        };
        Some((at, dirs, visible, scale))
    })();
    for side in 0..2 {
        let Some(n) = w.arrows.nodes[side].and_then(|id| w.ui.get_mut(id)) else {
            continue;
        };
        let Some((at, dirs, _, scale)) = shown.filter(|s| s.2[side]) else {
            n.set_visible(false);
            continue;
        };
        let d = dirs[side];
        // Clockwise from straight up, in the 32 steps of the arrow cells.
        let angle = d.x.atan2(-d.y).rem_euclid(std::f32::consts::TAU);
        let k = (angle / std::f32::consts::TAU * ARROW_DIRECTIONS as f32).round() as usize
            % ARROW_DIRECTIONS;
        if let Some(h) = w.arrows.cells.get(k) {
            n.set_image(h);
        }
        let half = ARROW_SIZE * scale / 2.0;
        let centre = at + d * ARROW_DISTANCE * scale;
        n.node = Node {
            left: px(centre.x - half),
            top: px(centre.y - half),
            width: px(half * 2.0),
            height: px(half * 2.0),
            ..n.node
        };
        n.set_visible(true);
    }
}

/// Drops the highlight when its lemming leaves play or a new level starts.
pub(crate) fn clear_highlight(w: &mut World) {
    let (game, current, lemming_cam) = (&w.game, &w.current, &w.lemming_cam);
    let highlight = &mut w.highlight;
    if current.loaded.is_none() {
        *highlight = Highlight::default();
        return;
    }
    let alive = |i: usize| {
        game.sim
            .as_ref()
            .and_then(|s| s.lemmings.get(i))
            .is_some_and(|l| !l.gone)
    };
    // The highlighted lemming gone, highlighting ends; riding along, the
    // view (and the highlight) go on to another lemming, and with none left
    // the arrow stays on (verified).
    if lemming_cam.following().is_none() && highlight.lemming.is_some_and(|i| !alive(i)) {
        *highlight = Highlight::default();
    }
}

/// The lemming on screen nearest the middle of the view, the one the arrow
/// highlights when switched on (seen in the original: the one nearest the
/// view's centre).
pub fn nearest_to_centre(
    sim: &l3d_sim::Simulation,
    camera: &SceneCamera,
    size: Vec2,
) -> Option<usize> {
    let mid = size / 2.0;
    sim.lemmings
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.gone)
        .filter_map(|(i, l)| {
            let (p, _) = to_screen(camera, size, lemming_centre(l))?;
            (p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y)
                .then_some((i, p.distance(mid)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The lemming's right and its back on screen (y down) with its head turned
/// `head` radians to the right: ahead of the eyes is up the screen, so
/// looking a quarter turn right its right is up.
fn riding_axes(head: f32) -> (Vec2, Vec2) {
    let (s, c) = head.sin_cos();
    (Vec2::new(c, -s), Vec2::new(s, c))
}

/// Whether screen point `p` is on the lemming's right side, the screen
/// split through its centre along the lemming's heading as seen with the
/// head turned `head` radians to the right.
fn riding_right_half(head: f32, size: Vec2, p: Vec2) -> bool {
    (p - size / 2.0).dot(riding_axes(head).0) >= 0.0
}

/// A lemming's turn to its left and to its right (facing (x, z) with y up,
/// the right hand is (−z, x): the clockwise turn).
fn lemming_sides(l: &l3d_sim::Lemming) -> [l3d_sim::Dir; 2] {
    [l.dir.anticlockwise(), l.dir.clockwise()]
}
