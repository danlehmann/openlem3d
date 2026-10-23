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

use l3d_engine::prelude::*;
use l3d_formats::screen;

use crate::menu::AppState;
use crate::texture::indexed_image;
use crate::ui::{Interaction, Node, NodeId, Ui, UiNode, percent};
use crate::{Data, World};

const SCREEN: Vec2 = Vec2::new(640.0, 480.0);
/// Text is the panel's large font at this scale (estimated from captures).
const TEXT_SCALE: f32 = 1.5;
const TOP_Y: f32 = 6.0;
const PROMPT_Y: f32 = 438.0;
const LABEL_Y: f32 = 445.0;

/// The briefing's screen and the preview's overlay, their choices, the
/// scene pictures loaded so far (by theme), and the preview's centre.
#[derive(Default)]
pub(crate) struct Briefing {
    root: Option<NodeId>,
    overlay: Option<NodeId>,
    choices: Vec<(NodeId, Choice)>,
    cache: HashMap<u8, Option<ScenePics>>,
    centre: Option<(u32, Vec3, bool)>,
}

impl Briefing {
    /// Whether `c` is pressed.
    fn on(&self, ui: &Ui, c: Choice) -> bool {
        self.choices.iter().any(|&(id, k)| k == c && ui.pressed(id))
    }

    /// Whether the pointer is on a choice.
    fn on_prompt(&self, ui: &Ui) -> bool {
        self.choices
            .iter()
            .any(|&(id, _)| ui.interaction(id) != Interaction::None)
    }

    fn scene(
        &mut self,
        data: &mut Data,
        theme: u8,
        images: &mut Assets<Image>,
    ) -> Option<ScenePics> {
        self.cache
            .entry(theme)
            .or_insert_with(|| load_scene(data, theme, images))
            .clone()
    }
}

/// Leaving the briefing takes its screen away.
pub(crate) fn exit(w: &mut World) {
    w.ui.remove_opt(&mut w.briefing.root);
}

#[derive(Clone)]
pub(crate) struct ScenePics {
    pub(crate) picture: Handle<Image>,
    pub(crate) prompts: Vec<Handle<Image>>,
}

/// Loads `SCENEnnn.SVG` (640×480) with its palette.
pub(crate) fn load_scene(
    data: &mut Data,
    theme: u8,
    images: &mut Assets<Image>,
) -> Option<ScenePics> {
    let d = &mut data.0;
    let scene = screen::scene(
        &d.read(&format!("GFX/SCENE{theme:03}.SVG")).ok()?,
        screen::HIGH_RES,
    )
    .ok()?;
    let pal = d.palette(&format!("GFX/SCENE{theme:03}.SVP")).ok()?;
    Some(ScenePics {
        picture: images.add(indexed_image(&scene.image, &pal, false)),
        // The prompts are drawn over the picture, index 0 transparent (as seen
        // in the original: the mice show the picture around them).
        prompts: scene
            .prompts
            .iter()
            .map(|p| images.add(indexed_image(p, &pal, true)))
            .collect(),
    })
}

/// Builds the briefing when it is shown and doesn't exist yet.
pub(crate) fn spawn_briefing(w: &mut World) {
    if w.briefing.root.is_some() {
        return;
    }
    let n = w.current.number;
    // A Practice briefing is the flyover itself, with the details and the
    // Demo prompt over it, not the theme picture (`docs/spec/camera.md`).
    if has_demo(n) {
        w.preview = Preview {
            active: true,
            practice: true,
            ..default()
        };
        w.next_state = Some(AppState::Playing);
        return;
    }
    let Ok(level) = w.data.0.level(n) else {
        return;
    };
    let pics = w.briefing.scene(&mut w.data, level.theme, &mut w.e.images);
    let ui = &mut w.ui;
    let root = ui.root(
        UiNode::new()
            .layout(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .background(Color::BLACK),
    );
    w.briefing.root = Some(root);
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    if let Some(p) = &pics {
        ui.child(
            canvas,
            UiNode::image(p.picture.clone()).at(0.0, 0.0, SCREEN.x, SCREEN.y),
        );
        // Left button, right button, the two halves of the ENTER key.
        let prompt = |i: usize| p.prompts.get(i).cloned();
        for (i, x) in [(0, 25.0), (2, 268.0), (3, 300.0), (1, 522.0)] {
            if let Some(h) = prompt(i) {
                ui.child(canvas, UiNode::image(h).at(x, PROMPT_Y, 32.0, 32.0));
            }
        }
    }
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let mut text = |s: &str, x: f32, y: f32, right_aligned: bool| {
        let (glyphs, width) = font.layout(s);
        let left = if right_aligned {
            x - width * TEXT_SCALE
        } else {
            x
        };
        for (gx, image) in glyphs {
            ui.child(
                canvas,
                UiNode::image(image).at(
                    left + gx * TEXT_SCALE,
                    y,
                    font.size.x * TEXT_SCALE,
                    font.size.y * TEXT_SCALE,
                ),
            );
        }
    };
    text("Level", 21.0, TOP_Y, false);
    text(&(n + 1).to_string(), 158.0, TOP_Y, true);
    text(level.title.trim(), 169.0, TOP_Y, false);
    text("Continue", 61.0, LABEL_Y, false);
    text(
        if has_demo(n) { "Demo" } else { "Preview" },
        342.0,
        LABEL_Y,
        false,
    );
    text("Menu", 561.0, LABEL_Y, false);
    // Clicking or tapping a prompt picks it; anywhere else continues.
    for (choice, x, width) in [
        (Choice::Preview, 268.0, 150.0),
        (Choice::Menu, 522.0, 110.0),
    ] {
        let id =
            w.ui.child(canvas, UiNode::new().button().at(x, PROMPT_Y, width, 32.0));
        w.briefing.choices.push((id, choice));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Preview,
    Menu,
}

pub(crate) fn briefing_input(w: &mut World) {
    let lifted = crate::touch::lifted(w);
    let b = &mut w.briefing;
    b.choices.retain(|&(id, _)| w.ui.contains(id));
    let (keys, mouse) = (&w.e.input.keys, &w.e.input.mouse);
    let on_prompt = b.on_prompt(&w.ui);
    let back = mouse.just_pressed(MouseButton::Right)
        || keys.just_pressed(KeyCode::Escape)
        || b.on(&w.ui, Choice::Menu);
    let show = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || b.on(&w.ui, Choice::Preview);
    let go = (mouse.just_pressed(MouseButton::Left) && !on_prompt)
        || keys.just_pressed(KeyCode::Space)
        || (lifted && !on_prompt)
        || keys
            .get_just_pressed()
            .any(|k| !matches!(k, KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter));
    if back {
        w.next_state = Some(crate::menu::level_menu(w.current.number));
    } else if show {
        w.preview = Preview {
            active: true,
            ..default()
        };
        w.next_state = Some(AppState::Playing);
    } else if go {
        w.next_state = Some(AppState::Playing);
    }
}

#[derive(Default)]
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
    let visible = |id: u8| {
        blocks
            .defs
            .get(id as usize)
            .is_some_and(|d| !d.is_placeholder() && d.faces.iter().any(|f| f.texture != 0xFF))
    };
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for (x, _, z, b, _) in level.cells() {
        if !b.is_empty() && visible(b.id) {
            lo = lo.min(Vec2::new(x as f32, z as f32));
            hi = hi.max(Vec2::new(x as f32 + 1.0, z as f32 + 1.0));
        }
    }
    let mid = if lo.x <= hi.x {
        (lo + hi) / 2.0
    } else {
        Vec2::splat(16.0)
    };
    Vec3::new(mid.x, UNSET_HEIGHT, mid.y)
}

pub(crate) fn preview(w: &mut World) {
    if !w.preview.active {
        return;
    }
    w.briefing.choices.retain(|&(id, _)| w.ui.contains(id));
    let lifted = crate::touch::lifted(w);
    let (on_menu, on_preview, on_prompt) = (
        w.briefing.on(&w.ui, Choice::Menu),
        w.briefing.on(&w.ui, Choice::Preview),
        w.briefing.on_prompt(&w.ui),
    );
    let on = |c: Choice| match c {
        Choice::Menu => on_menu,
        Choice::Preview => on_preview,
    };
    let time = w.e.time.game;
    let (keys, mouse) = (&mut w.e.input.keys, &mut w.e.input.mouse);
    let (current, state, game, data) = (&mut w.current, &mut w.preview, &mut w.game, &mut w.data);
    let (view, preset, next) = (&mut w.view, &mut w.preset, &mut w.next_state);
    let centre = &mut w.briefing.centre;
    // Right click (or Esc) goes back to the level list, as in the original;
    // any other key, click or tap starts the level.
    if state.started
        && (mouse.just_pressed(MouseButton::Right)
            || keys.just_pressed(KeyCode::Escape)
            || on(Choice::Menu))
    {
        state.active = false;
        game.paused = false;
        current.loaded = None;
        *next = Some(crate::menu::level_menu(current.number));
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
        let log = data
            .0
            .level(n)
            .ok()
            .zip(data.0.blocks(n).ok())
            .and_then(|(level, blocks)| l3d_sim::demos::demo(n, &level, &blocks));
        state.active = false;
        game.paused = false;
        game.replay = log.map(|log| crate::Replay::demo(log, AppState::Briefing));
        current.loaded = None;
        return;
    }
    let any = keys.get_just_pressed().next().is_some()
        || (mouse.get_just_pressed().next().is_some() && !on_prompt)
        || (lifted && !on_prompt);
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
            preset.set(0);
        }
        state.gliding = true;
        return;
    }
    game.paused = true;
    let first = !state.started;
    // Input that started the preview doesn't end it.
    state.started = true;
    let Some((level, blocks, _)) = game.terrain.as_ref() else {
        return;
    };
    if centre.is_none_or(|(n, ..)| n != current.number) || first {
        // It starts where camera 1 looks from.
        state.angle = level.cameras[0].rotation as f32 * std::f32::consts::FRAC_PI_2;
        *centre = Some((
            current.number,
            preview_centre(level, blocks),
            level.flags & FLAG_PREVIEW_STATIC != 0,
        ));
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
pub(crate) fn end_demo(w: &mut World) {
    let Some(back_to) = w.game.replay.as_ref().and_then(|r| r.demo) else {
        return;
    };
    let lifted = crate::touch::lifted(w);
    let (keys, mouse) = (&mut w.e.input.keys, &mut w.e.input.mouse);
    if keys.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some()
        || lifted
    {
        keys.clear();
        mouse.clear();
        w.game.replay = None;
        w.current.loaded = None;
        w.next_state = Some(back_to);
    }
}

fn has_demo(level: u32) -> bool {
    level >= crate::menu::PRACTICE as u32 * crate::menu::LEVELS_PER_RATING
        && l3d_sim::demos::solution(level).is_some()
}

/// Any key, click or tap ends a demo, back to where it was started.
fn preview_lines(n: u32, level: &l3d_formats::level::Level) -> [String; 6] {
    let rating = crate::menu::RATINGS
        .get((n / crate::menu::LEVELS_PER_RATING) as usize)
        .copied()
        .unwrap_or("");
    [
        format!("Level {}  {}", n + 1, level.title.trim()),
        format!("Number Of Lemmings {}", level.lemmings),
        format!("{} To Be Saved", level.save_requirement),
        format!("Release Rate {}", level.release_rate),
        format!(
            "Time {}:{:02} Minutes",
            level.time_minutes, level.time_seconds
        ),
        format!("Rating {rating}"),
    ]
}

pub(crate) fn preview_overlay(w: &mut World) {
    let state = &w.preview;
    if !state.active || state.gliding {
        w.ui.remove_opt(&mut w.briefing.overlay);
        return;
    }
    // Until the level is loaded, the terrain is still the previous level's.
    let n = w.current.number;
    if w.briefing.overlay.is_some() || w.current.loaded != Some(n) {
        return;
    }
    let (Some(font), Some((level, ..))) = (
        w.art.as_ref().and_then(|a| a.large.as_ref()),
        w.game.terrain.as_ref(),
    ) else {
        return;
    };
    let practice = state.practice;
    let ui = &mut w.ui;
    let root = ui.root(UiNode::new().layout(Node {
        width: percent(100),
        height: percent(100),
        ..default()
    }));
    w.briefing.overlay = Some(root);
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    let text = |ui: &mut Ui, s: &str, x: f32, y: f32| {
        let (glyphs, _) = font.layout(s);
        // The shadow first, then the text.
        for (shadow, dx) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
            for (gx, image) in glyphs.iter() {
                let mut g = UiNode::image(image.clone()).at(
                    x + gx * TEXT_SCALE + dx,
                    y + dx,
                    font.size.x * TEXT_SCALE,
                    font.size.y * TEXT_SCALE,
                );
                if let Some(c) = shadow {
                    g = g.tint(c);
                }
                ui.child(canvas, g);
            }
        }
    };
    for (k, line) in preview_lines(n, level).iter().enumerate() {
        text(ui, line, 21.0, TOP_Y + 26.0 * k as f32);
    }
    let pics = w.briefing.scene(&mut w.data, level.theme, &mut w.e.images);
    if let Some(p) = &pics {
        // Left button, right button and, on a Practice level, the two halves
        // of the ENTER key.
        let enter: &[(usize, f32)] = if practice {
            &[(2, 268.0), (3, 300.0)]
        } else {
            &[]
        };
        for &(i, x) in [(0, 25.0), (1, 522.0)].iter().chain(enter) {
            if let Some(h) = p.prompts.get(i).cloned() {
                ui.child(canvas, UiNode::image(h).at(x, PROMPT_Y, 32.0, 32.0));
            }
        }
    }
    text(ui, "Continue", 61.0, LABEL_Y);
    text(ui, "Menu", 561.0, LABEL_Y);
    if practice {
        text(ui, "Demo", 342.0, LABEL_Y);
    }
    // Clicking or tapping a prompt picks it; anywhere else continues.
    let buttons: &[(Choice, f32, f32)] = if practice {
        &[
            (Choice::Preview, 268.0, 150.0),
            (Choice::Menu, 522.0, 110.0),
        ]
    } else {
        &[(Choice::Menu, 522.0, 110.0)]
    };
    for &(choice, x, width) in buttons {
        let id = ui.child(canvas, UiNode::new().button().at(x, PROMPT_Y, width, 32.0));
        w.briefing.choices.push((id, choice));
    }
}
