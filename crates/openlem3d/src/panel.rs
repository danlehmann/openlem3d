//! The in-level panel, laid out like the original's (`docs/spec/ui-graphics.md`,
//! "In-level panel"): sprites from `ICONS.RNC` and `MINILEMM.GFX` drawn over
//! the 3D view at the original's 320×200 positions, scaled to the window
//! height. On wider windows the right-hand column keeps to the right edge and
//! the skill row to the bottom left.

use std::collections::HashMap;

use l3d_engine::prelude::*;
use l3d_formats::font::Font;
use l3d_formats::icons::{Icons, panel as icon};
use l3d_formats::image::IndexedImage;
use l3d_formats::sheets;
use l3d_sim::Skill;

use crate::lemming_cam::LemmingCam;
use crate::menu::AppState;
use crate::screen::Screen;
use crate::texture::indexed_image;
use crate::ui::{Interaction, Node, NodeId, Ui, UiNode, percent, px};
use crate::{Game, World};

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
/// The bomb's mushroom cloud, looped after a nuke (rate ours).
const EXPLOSION_FPS: f32 = 14.0;
/// The longest gap between the two clicks of a double click (ours).
const DOUBLE_CLICK_SECONDS: f32 = 0.5;

/// Panel graphics, uploaded once.
#[derive(Clone)]
struct Art {
    panel: Vec<Handle<Image>>,
    labels: Vec<Handle<Image>>,
    umbrella: Handle<Image>,
    /// The height bar and its knob, opaque.
    height_bar: Handle<Image>,
    knob: Handle<Image>,
    skills: Vec<Handle<Image>>,
    small: Glyphs,
    large: Glyphs,
}

/// A font's glyph images by character code, and its advance per character.
#[derive(Clone)]
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
        if c == ':' {
            self.colon_advance
        } else {
            self.digit_advance
        }
    }
}

pub(crate) fn load_art(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else {
        return;
    };
    let Some(icons) = d
        .read("GFX/ICONS.RNC")
        .ok()
        .and_then(|raw| Icons::parse(&raw).ok())
    else {
        return;
    };
    let Some(skills) = d
        .read(sheets::MINILEMM.path)
        .ok()
        .and_then(|raw| sheets::MINILEMM.cut(&raw).ok())
    else {
        return;
    };
    let height_bar = images.add(indexed_image(&icons.height_bar, &pal, false));
    let knob = images.add(indexed_image(&icons.knob, &pal, false));
    let mut add = |img: &IndexedImage| images.add(indexed_image(img, &pal, true));
    let glyphs = |f: &Font,
                  digit_advance: f32,
                  colon_advance: f32,
                  add: &mut dyn FnMut(&IndexedImage) -> Handle<Image>| Glyphs {
        first: f.first,
        images: f.glyphs.iter().map(&mut *add).collect(),
        size: Vec2::new(f.glyph_width as f32, f.glyph_height as f32),
        digit_advance,
        colon_advance,
    };
    let small = glyphs(&icons.small_font, 5.0, 3.0, &mut add);
    let large = glyphs(&icons.large_font, 7.0, 4.0, &mut add);
    w.panel.art = Some(Art {
        panel: icons.panel.iter().map(&mut add).collect(),
        labels: icons.labels.iter().map(&mut add).collect(),
        umbrella: add(&icons.umbrella),
        height_bar,
        knob,
        skills: skills.iter().map(&mut add).collect(),
        small,
        large,
    });
}

/// Where an element sits: its top-left corner in the original's 320×200
/// screen, and its size there.
#[derive(Clone, Copy)]
struct PanelPos {
    at: Vec2,
    size: Vec2,
}

/// What clicking an element does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
#[derive(Clone, Copy)]
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

/// Elements whose picture changes with the game state.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    Nuke,
}

/// The panel: its pictures, its root and items (with their places), the
/// buttons' actions, the animated items, the counters' glyph slots, the
/// height bar, knob, minimap and its frame, the caption with its text, and
/// what its systems keep between frames.
#[derive(Default)]
pub(crate) struct Panel {
    art: Option<Art>,
    root: Option<NodeId>,
    pos: HashMap<NodeId, PanelPos>,
    actions: Vec<(NodeId, Action)>,
    animated: Vec<(NodeId, Animated)>,
    texts: Vec<(GlyphText, Vec<NodeId>)>,
    bar: Option<NodeId>,
    knob: Option<NodeId>,
    map: Option<NodeId>,
    frame: Option<NodeId>,
    caption: Option<(NodeId, String)>,
    last_bomb_click: Option<f32>,
    carry: f32,
    nuked_at: Option<f32>,
    /// A skill given straight to the highlighted lemming, and when (real
    /// seconds): its button plays its animation once, though nothing is
    /// selected (owner's observation).
    flash: Option<(Skill, f32)>,
    /// The writes to the selected skill seen.
    select_seen: u64,
}

/// Builds the panel: its items, each placed by [`PanelPos`].
struct Builder<'a> {
    ui: &'a mut Ui,
    panel: &'a mut Panel,
    root: NodeId,
}

impl Builder<'_> {
    fn item(
        &mut self,
        at: Vec2,
        size: Vec2,
        image: Handle<Image>,
        action: Option<Action>,
        anim: Option<Animated>,
    ) -> NodeId {
        let mut n = UiNode::image(image);
        if action.is_some() {
            n = n.button();
        }
        let id = self.ui.child(self.root, n);
        self.panel.pos.insert(id, PanelPos { at, size });
        if let Some(a) = action {
            self.panel.actions.push((id, a));
        }
        if let Some(a) = anim {
            self.panel.animated.push((id, a));
        }
        id
    }

    fn text(&mut self, art: &Art, large: bool, right: f32, y: f32, value: Value, slots: usize) {
        let glyphs = if large { &art.large } else { &art.small };
        let t = self.ui.child(self.root, UiNode::new());
        let mut ids = Vec::new();
        for i in 0..slots {
            let at = Vec2::new(right - glyphs.digit_advance * i as f32, y);
            let s = self.ui.child(t, UiNode::image(Handle::default()).hidden());
            self.panel.pos.insert(
                s,
                PanelPos {
                    at,
                    size: glyphs.size,
                },
            );
            ids.push(s);
        }
        self.panel.texts.push((
            GlyphText {
                large,
                right,
                y,
                value,
            },
            ids,
        ));
    }
}

pub(crate) fn spawn_panel(w: &mut World) {
    if w.panel.art.is_none() {
        return;
    }
    let frame = w.e.images.add(crate::minimap::frame_image());
    let root = w.ui.root(UiNode::new().layout(Node {
        width: percent(100),
        height: percent(100),
        ..default()
    }));
    let mut panel = std::mem::take(&mut w.panel);
    let art = panel.art.take().expect("checked above");
    let art = &art;
    panel.root = Some(root);
    let mut b = Builder {
        ui: &mut w.ui,
        panel: &mut panel,
        root,
    };
    // The minimap and its frame (the map image arrives with the level).
    let icon24 = Vec2::splat(24.0);
    let p = |i: usize| art.panel[i].clone();
    let frame = b.item(Vec2::ZERO, Vec2::splat(70.0), frame, None, None);
    b.panel.frame = Some(frame);
    let map = b.item(
        Vec2::splat(3.0),
        Vec2::splat(64.0),
        Handle::default(),
        None,
        None,
    );
    if let Some(n) = b.ui.get_mut(map) {
        *n = std::mem::take(n).button().hidden();
    }
    b.panel.map = Some(map);
    // Right-hand column.
    b.item(
        Vec2::new(260.0, 0.0),
        Vec2::new(32.0, 16.0),
        art.labels[0].clone(),
        None,
        None,
    );
    b.item(
        Vec2::new(260.0, 16.0),
        Vec2::new(32.0, 16.0),
        art.labels[1].clone(),
        None,
        None,
    );
    b.item(
        Vec2::new(260.0, 32.0),
        Vec2::new(32.0, 16.0),
        art.labels[2].clone(),
        None,
        None,
    );
    b.item(
        Vec2::new(260.0, 48.0),
        icon24,
        p(icon::BOMB),
        Some(Action::Nuke),
        Some(Animated::Nuke),
    );
    b.item(
        Vec2::new(284.0, 48.0),
        icon24,
        p(icon::PLAY),
        Some(Action::FastForward),
        Some(Animated::FastForward),
    );
    b.item(
        Vec2::new(260.0, 72.0),
        icon24,
        p(icon::CAMERA),
        Some(Action::Camera),
        None,
    );
    b.item(
        Vec2::new(284.0, 72.0),
        icon24,
        p(icon::PAUSE.start),
        Some(Action::Pause),
        Some(Animated::Paws),
    );
    b.item(
        Vec2::new(260.0, 96.0),
        icon24,
        p(icon::TURN_CLOCKWISE + 1),
        Some(Action::TurnClockwise),
        Some(Animated::TurnClockwise),
    );
    b.item(
        Vec2::new(284.0, 96.0),
        icon24,
        p(icon::TURN_ANTICLOCKWISE + 1),
        Some(Action::TurnAnticlockwise),
        Some(Animated::TurnAnticlockwise),
    );
    b.item(
        Vec2::new(260.0, 120.0),
        icon24,
        p(icon::MINUS + 1),
        Some(Action::Slower),
        Some(Animated::Slower),
    );
    b.item(
        Vec2::new(284.0, 120.0),
        icon24,
        p(icon::PLUS + 1),
        Some(Action::Faster),
        Some(Animated::Faster),
    );
    b.item(
        Vec2::new(274.0, 143.0),
        Vec2::splat(32.0),
        art.umbrella.clone(),
        None,
        None,
    );
    // The height bar down the right edge, a button so that it is not part
    // of the view.
    let bar = b.item(
        Vec2::new(BAR_X, 0.0),
        Vec2::new(12.0, SCREEN.y),
        art.height_bar.clone(),
        None,
        None,
    );
    if let Some(n) = b.ui.get_mut(bar) {
        *n = std::mem::take(n).button();
    }
    b.panel.bar = Some(bar);
    let knob = b.item(
        Vec2::new(BAR_X + 2.0, 0.0),
        Vec2::new(9.0, 8.0),
        art.knob.clone(),
        None,
        None,
    );
    b.panel.knob = Some(knob);
    // Bottom row.
    b.item(
        Vec2::new(0.0, 172.0),
        icon24,
        p(icon::RED_DOWN),
        Some(Action::Arrow),
        Some(Animated::Arrow),
    );
    b.item(
        Vec2::new(24.0, 172.0),
        icon24,
        p(icon::FACE + 1),
        Some(Action::LemmingCam),
        Some(Animated::Face),
    );
    for (skill, (x, cells)) in Skill::ALL.into_iter().zip(SKILL_BUTTONS) {
        b.item(
            Vec2::new(x, SKILL_Y),
            Vec2::splat(32.0),
            art.skills[cells[0]].clone(),
            Some(Action::Skill(skill)),
            Some(Animated::Skill(skill)),
        );
    }
    // Numbers.
    b.text(art, true, 299.0, 2.0, Value::In, 3);
    b.text(art, true, 299.0, 18.0, Value::Out, 3);
    b.text(art, true, 299.0, 34.0, Value::Time, 4);
    b.text(art, false, 272.0, 140.0, Value::MinRate, 2);
    b.text(art, false, 296.0, 140.0, Value::Rate, 2);
    b.text(art, false, 269.0, 80.0, Value::CameraNumber, 1);
    for (skill, (x, _)) in Skill::ALL.into_iter().zip(SKILL_BUTTONS) {
        b.text(art, false, x + 17.0, 188.0, Value::SkillCount(skill), 2);
    }
    let c = b.ui.child(root, UiNode::new());
    b.panel.caption = Some((c, String::new()));
    w.minimap_view.0 = panel.map;
    panel.art = Some(art.clone());
    w.panel = panel;
}

/// Screen pixels per original pixel.
fn scale(screen: &Screen) -> f32 {
    (screen.size.y / SCREEN.y).max(1.0)
}

/// Places every element: the skill row keeps to the bottom left, the
/// right-hand column (x ≥ 256) to the top right and the height bar to the
/// right edge, the rest to the top left.
fn layout(w: &mut World) {
    let s = scale(&w.screen);
    let (width, height) = (w.screen.size.x, w.screen.size.y);
    let panel = &mut w.panel;
    panel.pos.retain(|id, _| w.ui.contains(*id));
    for (&id, p) in &panel.pos {
        let Some(n) = w.ui.get_mut(id) else { continue };
        let x = if p.at.x >= BAR_X || (p.at.y < 160.0 && p.at.x >= 256.0) {
            width - (SCREEN.x - p.at.x) * s
        } else {
            p.at.x * s
        };
        let y = if p.at.y >= 160.0 {
            height - (SCREEN.y - p.at.y) * s
        } else {
            p.at.y * s
        };
        n.node.left = px(x);
        n.node.top = px(y);
        n.node.width = px(p.size.x * s);
        n.node.height = px(p.size.y * s);
    }
}

fn buttons(w: &mut World) {
    let now = w.e.time.real.elapsed_secs();
    let (selected, game, lemming_cam, view) = (
        &mut w.selected,
        &mut w.game,
        &mut w.lemming_cam,
        &mut w.view,
    );
    let (cameras, preset, sfx, highlight) = (&w.info, &mut w.preset, &mut w.sfx, &mut w.highlight);
    let (pending_turner, scene_camera, screen) = (&mut w.pending, &w.camera, &w.screen);
    let panel = &mut w.panel;
    let pressed: Vec<Action> = panel
        .actions
        .iter()
        .filter(|&&(id, _)| w.ui.just_pressed(id))
        .map(|&(_, a)| a)
        .collect();
    let time = w.e.time.real;
    let _ = now;
    for action in pressed {
        let paused = game.paused;
        let Some(sim) = &mut game.sim else { return };
        match action {
            Action::Skill(s) => {
                // A skill with none left can't be selected (verified). Paused,
                // it is only selected, not given to the highlighted lemming.
                if sim.skills_left[s as usize] > 0 {
                    match highlight.lemming.filter(|_| !paused) {
                        // Given straight to the highlighted lemming, without
                        // selecting the skill (seen in the original); its
                        // button plays its animation once, and the next click
                        // highlights again (owner's observation). A turner
                        // then waits for the click on the side it points to,
                        // still with nothing selected.
                        Some(i) if s == Skill::Turner => {
                            if sim.can_assign(i, s) {
                                pending_turner.0 = Some(i);
                                highlight.last_skill = Some(s);
                                panel.flash = Some((s, time.elapsed_secs()));
                            }
                        }
                        Some(i) => {
                            let ok = sim.assign(i, s);
                            if ok {
                                highlight.last_skill = Some(s);
                                panel.flash = Some((s, time.elapsed_secs()));
                            }
                            sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
                        }
                        None => selected.set(Some(s)),
                    }
                }
            }
            // Only a double click on the bomb nukes (owner's recollection);
            // Alt+Q works at once.
            Action::Nuke => {
                let now = time.elapsed_secs();
                if panel
                    .last_bomb_click
                    .is_some_and(|t| now - t <= DOUBLE_CLICK_SECONDS)
                {
                    panel.last_bomb_click = None;
                    if !sim.nuked {
                        sfx.play("VOXFX/GEDDON1");
                    }
                    sim.nuke();
                } else {
                    panel.last_bomb_click = Some(now);
                }
            }
            Action::FastForward => game.fast_forward = !game.fast_forward,
            Action::Pause => game.paused = !game.paused,
            Action::Camera => {
                if let Some(cams) = cameras {
                    preset.set((preset.0 + 1) % 4);
                    let (pos, yaw) = cams.slots[preset.0];
                    view.glide_to(pos, yaw);
                    *lemming_cam = LemmingCam::Off;
                }
            }
            // The arrow arms highlighting (the next click on a lemming picks
            // it) and deselects the skill, as a skill button then gives its
            // skill to the highlighted lemming; clicked again, it disarms and
            // drops the highlight. In the lemming view it moves on to the
            // next lemming in play and rides along with it (owner's
            // observation).
            Action::Arrow => {
                if let Some(i) = lemming_cam.following() {
                    let n = sim.lemmings.len();
                    let next = (1..=n)
                        .map(|k| (i + k) % n)
                        .find(|&j| !sim.lemmings[j].gone)
                        .unwrap_or(i);
                    lemming_cam.ride(next, highlight, selected, view);
                } else if highlight.on {
                    // The last skill given while highlighting becomes the
                    // selected one (owner's observation).
                    selected.set(highlight.last_skill);
                    *highlight = crate::hud::Highlight::default();
                } else {
                    selected.set(None);
                    let size = screen.size;
                    *highlight = crate::hud::Highlight {
                        on: true,
                        lemming: crate::hud::nearest_to_centre(sim, scene_camera, size),
                        last_skill: None,
                    };
                }
            }
            Action::LemmingCam => {
                lemming_cam.face(highlight, selected, view);
            }
            Action::TurnClockwise | Action::TurnAnticlockwise | Action::Slower | Action::Faster => {
            }
        }
    }
}

/// Holding − or + changes the release rate by one per 70 Hz frame, with no
/// initial delay (verified); the turn arrows turn the world about the
/// level's centre while held (owner's observation: they turn the whole
/// level, not the camera), or turn the head riding along with a lemming.
fn held_buttons(w: &mut World) {
    let (keys, time, ui, panel) = (&w.e.input.keys, w.e.time.game, &w.ui, &mut w.panel);
    let (game, lemming_cam, view) = (&mut w.game, &w.lemming_cam, &mut w.view);
    const RATE_STEPS_PER_SECOND: f32 = 70.0;
    let down = |a: Action| {
        panel
            .actions
            .iter()
            .any(|&(id, b)| b == a && ui.pressed(id))
    };
    let turn =
        down(Action::TurnAnticlockwise) as i32 as f32 - down(Action::TurnClockwise) as i32 as f32;
    let slower = down(Action::Slower)
        || keys.pressed(KeyCode::Minus)
        || keys.pressed(KeyCode::NumpadSubtract);
    let faster =
        down(Action::Faster) || keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd);
    if let Some(sim) = &mut game.sim {
        let first = keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::Equal);
        if slower != faster {
            panel.carry += time.delta_secs() * RATE_STEPS_PER_SECOND;
            // A tap changes the rate by at least one.
            let steps = if first {
                (panel.carry as i32).max(1)
            } else {
                panel.carry as i32
            };
            panel.carry -= steps as f32;
            sim.adjust_release_rate(if faster { steps } else { -steps });
        } else {
            panel.carry = 0.0;
        }
    }
    // Riding along, they turn the head instead.
    if turn != 0.0 {
        if lemming_cam.following().is_none() {
            if let Some(sim) = game.sim.as_ref() {
                view.orbit(sim.centre(), -turn * crate::ORBIT_SPEED * time.delta_secs());
            }
        } else {
            view.head = (view.head - turn * crate::lemming_cam::HEAD_SPEED * time.delta_secs())
                .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        }
    }
}

/// The height bar's column (original pixels).
const BAR_X: f32 = 308.0;
/// The knob's yellow top on the bar is `BAR_TOP - BAR_SLOPE · h^1.5` for
/// camera height `h` (original pixels; fitted to the original's knob at the
/// presets of Practice "Rope Slide", heights 2.75 to 7.75, within a pixel; it
/// puts the bar's lowest knob at the camera's lowest height).
const BAR_TOP: f32 = 195.8;
const BAR_SLOPE: f32 = 4.6;
/// The knob's yellow square starts a row below the knob, and is 6 rows tall;
/// the original centres it on the pointer.
const KNOB_YELLOW: f32 = 1.0;
const KNOB_CENTRE: f32 = 3.0;
/// How far the knob travels: its yellow top's range on the bar (measured).
const KNOB_RANGE: std::ops::RangeInclusive<f32> = 3.0..=190.0;

/// The knob's yellow top for camera height `h`.
fn knob_at(h: f32) -> f32 {
    (BAR_TOP - BAR_SLOPE * h.max(0.0).powf(1.5)).clamp(*KNOB_RANGE.start(), *KNOB_RANGE.end())
}

/// The camera height whose knob's yellow top is at `y`.
fn height_at(y: f32) -> f32 {
    ((BAR_TOP - y.clamp(*KNOB_RANGE.start(), *KNOB_RANGE.end())) / BAR_SLOPE).powf(2.0 / 3.0)
}

/// The height bar: the knob shows the camera's height. Holding the camera
/// button on the bar (in Enhanced mode, either button) moves the camera up
/// or down, at the camera keys' speed, to the height whose knob would sit
/// under the pointer (verified: the original's camera climbs and sinks
/// until the knob reaches the pointer; its left button does nothing there).
fn height_bar(w: &mut World) {
    let (window, screen, mouse) = (&w.e.window, &w.screen, &w.e.input.mouse);
    let (settings, time, lemming_cam) = (&w.settings, w.e.time.game, &w.lemming_cam);
    let view = &mut w.view;
    let held = mouse.pressed(settings.turn_button())
        || (settings.enhanced && mouse.pressed(settings.action_button()));
    let on_bar = w
        .panel
        .bar
        .is_some_and(|b| w.ui.interaction(b) != Interaction::None);
    if held
        && lemming_cam.following().is_none()
        && on_bar
        && let Some(p) = screen.cursor(window)
    {
        let target = height_at(p.y / scale(screen) - KNOB_CENTRE);
        let step = crate::CAMERA_SPEED * settings.camera_factor() * time.delta_secs();
        view.pos.y += (target - view.pos.y).clamp(-step, step);
        view.glide = None;
    }
    if let Some(knob) = w.panel.knob.and_then(|k| w.panel.pos.get_mut(&k)) {
        knob.at.y = knob_at(view.pos.y) - KNOB_YELLOW;
    }
}

/// F1–F9 also only select skills that are left.
fn skill_keys_select(w: &mut World) {
    let selected = &mut w.selected;
    let changed = w.panel.select_seen != selected.1;
    if let (Some(sim), Some(s)) = (&w.game.sim, selected.0)
        && sim.skills_left[s as usize] == 0
        && changed
    {
        selected.set(None);
    }
    w.panel.select_seen = selected.1;
}

/// A level starts with the first slot, the blocker, selected if it has any,
/// otherwise with nothing selected (level 2 started with the blocker; the
/// Practice "Bomber", with only bombers, with nothing, owner's observation).
pub(crate) fn select_first_skill(w: &mut World) {
    w.selected.set(
        w.game
            .sim
            .as_ref()
            .filter(|sim| sim.skills_left[Skill::Blocker as usize] > 0)
            .map(|_| Skill::Blocker),
    );
}

fn animate(w: &mut World) {
    let (time, real) = (w.e.time.game, w.e.time.real);
    let (game, selected, lemming_cam) = (&w.game, &w.selected, &w.lemming_cam);
    let (keys, highlight) = (&w.e.input.keys, &w.highlight);
    let (ui, panel) = (&mut w.ui, &mut w.panel);
    let Some(art) = &panel.art else { return };
    let t = time.elapsed_secs();
    let nuked = game.sim.as_ref().is_some_and(|s| s.nuked);
    match (nuked, panel.nuked_at) {
        (true, None) => panel.nuked_at = Some(t),
        (false, Some(_)) => panel.nuked_at = None,
        _ => {}
    }
    let nuked_at = panel.nuked_at;
    let down = |a: Action| {
        panel
            .actions
            .iter()
            .any(|&(id, b)| b == a && ui.pressed(id))
    };
    let mut images = Vec::new();
    for &(id, anim) in &panel.animated {
        let image = match anim {
            Animated::Skill(s) => {
                let cells = SKILL_BUTTONS[s as usize].1;
                // The selected skill animates while any are left, but never
                // alongside highlighting; a skill just given to the
                // highlighted lemming plays its animation once (owner's
                // observation).
                let left = game
                    .sim
                    .as_ref()
                    .is_some_and(|sim| sim.skills_left[s as usize] > 0);
                let once = panel
                    .flash
                    .filter(|&(f, _)| f == s)
                    .map(|(_, at)| ((real.elapsed_secs() - at) * SKILL_FPS / 4.0) as usize)
                    .filter(|&k| k < cells.len());
                let frame = if let Some(k) = once {
                    k
                } else if selected.0 == Some(s) && left && !highlight.on {
                    (t * SKILL_FPS / 4.0) as usize % cells.len()
                } else {
                    0
                };
                art.skills[cells[frame]].clone()
            }
            Animated::FastForward => art.panel[if game.fast_forward {
                icon::FAST_FORWARD
            } else {
                icon::PLAY
            }]
            .clone(),
            Animated::Paws => {
                let frame = if game.paused {
                    (t * PAWS_FPS) as usize % icon::PAUSE.len()
                } else {
                    0
                };
                art.panel[icon::PAUSE.start + frame].clone()
            }
            // Green (the lower cell) while held, red otherwise.
            Animated::TurnClockwise => {
                art.panel[icon::TURN_CLOCKWISE + !down(Action::TurnClockwise) as usize].clone()
            }
            Animated::TurnAnticlockwise => art.panel
                [icon::TURN_ANTICLOCKWISE + !down(Action::TurnAnticlockwise) as usize]
                .clone(),
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
            Animated::Arrow => match highlight.on {
                true => art.panel
                    [icon::GREEN_DOWN.start + (t * ARROW_FPS) as usize % icon::GREEN_DOWN.len()]
                .clone(),
                false => art.panel[icon::RED_DOWN].clone(),
            },
            // Squinting when idle, eyes open when armed or riding along.
            Animated::Face => {
                art.panel[icon::FACE + (*lemming_cam == LemmingCam::Off) as usize].clone()
            }
            // The mushroom cloud, over and over from the nuke until the level
            // ends (owner's recollection).
            Animated::Nuke => match nuked_at {
                Some(at) => art.panel[icon::EXPLOSION.start
                    + ((t - at) * EXPLOSION_FPS) as usize % icon::EXPLOSION.len()]
                .clone(),
                None => art.panel[icon::BOMB].clone(),
            },
        };
        images.push((id, image));
    }
    for (id, image) in images {
        ui.set_image(id, &image);
    }
}

/// The string a counter shows.
fn value_string(v: Value, game: &Game, preset: usize) -> String {
    let Some(sim) = &game.sim else {
        return String::new();
    };
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
fn glyph_texts(w: &mut World) {
    let panel = &mut w.panel;
    let Some(art) = &panel.art else { return };
    for (text, slots) in &panel.texts {
        let glyphs = if text.large { &art.large } else { &art.small };
        let s = value_string(text.value, &w.game, w.preset.0);
        // Positions from the right: each character's cell x.
        // Each character's cell starts its own advance left of the next one's
        // ("3:53": digits at 281, 292, 299 and the colon at 288).
        let mut placed: Vec<(f32, char)> = Vec::new();
        for c in s.chars().rev() {
            let x = placed
                .last()
                .map_or(text.right, |&(next_x, _)| next_x - glyphs.advance(c));
            placed.push((x, c));
        }
        for (i, &id) in slots.iter().enumerate() {
            let Some(n) = w.ui.get_mut(id) else { continue };
            match placed.get(i).and_then(|&(x, c)| Some((x, glyphs.get(c)?))) {
                Some((x, image)) => {
                    if let Some(p) = panel.pos.get_mut(&id) {
                        p.at = Vec2::new(x, text.y);
                    }
                    n.set_image(&image);
                    n.set_visible(true);
                }
                None => n.set_visible(false),
            }
        }
    }
}

/// Shows the level's minimap image and its frame, unless the level hides
/// its minimap; then neither shows (observed on `LEVEL.019`, `044`, `062`).
pub(crate) fn show_minimap(w: &mut World) {
    let Some(map) = &w.minimap else { return };
    if let Some(n) = w.panel.frame.and_then(|f| w.ui.get_mut(f)) {
        n.set_visible(map.shown);
    }
    if let Some(n) = w.panel.map.and_then(|m| w.ui.get_mut(m)) {
        n.set_image(&map.image);
        n.set_visible(map.shown);
    }
}

/// The panel shows while playing, but not during the level preview (an
/// interstitial in the original, without the panel).
pub(crate) fn show_in_play(w: &mut World) {
    let shown = w.state == AppState::Playing && !w.opts.no_hud && !w.preview.active;
    if let Some(root) = w.panel.root {
        w.ui.set_visible(root, shown);
    }
}

/// The line of large text at the bottom left, above the arrow and the face
/// (position estimated): "Replaying" and "Click to Play" during a replay,
/// otherwise the state of the lemming under the pointer, as in the original
/// ("Walker", "Sliding").
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

fn caption(w: &mut World) {
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let Some((node, shown)) = &mut w.panel.caption else {
        return;
    };
    let game = &w.game;
    let text = if game.replay.as_ref().is_some_and(|r| r.demo.is_some()) {
        "Demo"
    } else if game.replay.is_some() {
        if ((w.e.time.game.elapsed_secs() / REPLAY_BLINK) as u32).is_multiple_of(2) {
            "Replaying"
        } else {
            "Click to Play"
        }
    } else {
        (|| {
            let point = w.screen.cursor(&w.e.window)?;
            if w.ui.any_active() {
                return None;
            }
            let sim = game.sim.as_ref()?;
            // Riding along, not the lemming ridden with.
            let i = crate::hud::lemming_near(
                sim,
                &w.camera,
                w.screen.size,
                point,
                w.lemming_cam.following(),
            )?;
            state_name(&sim.lemmings[i])
        })()
        .unwrap_or("")
    };
    if shown == text {
        return;
    }
    *shown = text.to_string();
    let node = *node;
    for c in w.ui.children(node).to_vec() {
        w.ui.remove(c);
    }
    let (glyphs, _) = font.layout(text);
    for (x, image) in glyphs {
        let g = w.ui.child(node, UiNode::image(image));
        w.panel.pos.insert(
            g,
            PanelPos {
                at: CAPTION_AT + Vec2::X * x,
                size: font.size,
            },
        );
    }
}

/// The panel's systems while playing, in order.
pub(crate) fn update(w: &mut World) {
    layout(w);
    buttons(w);
    held_buttons(w);
    skill_keys_select(w);
    animate(w);
    glyph_texts(w);
    caption(w);
}

/// Before the camera moves: the height bar.
pub(crate) fn before_camera(w: &mut World) {
    height_bar(w);
}
