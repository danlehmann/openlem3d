//! The end-of-level screen: the level's theme picture, how many lemmings
//! were saved against how many were needed, the next level's password when
//! known, and two choices: Next Level (or Retry after a failure) on the left
//! button and Menu on the right, as in the original (navigation notes:
//! "Next Level"/"Retry" bottom left, "Menu" bottom right). The wording of
//! the result lines is our own. Input is taken at once.

use l3d_engine::prelude::*;

use crate::World;
use crate::menu::AppState;
use crate::ui::{Interaction, Node, NodeId, UiNode, percent};

#[derive(Clone, Copy)]
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
/// The outcome being shown, and the screen showing it.
#[derive(Default)]
pub(crate) struct Results {
    outcome: Option<Outcome>,
    root: Option<NodeId>,
    choices: Vec<(NodeId, Choice)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Next,
    Menu,
}

pub(crate) fn level_over(w: &mut World) {
    let (game, current) = (&w.game, &mut w.current);
    let Some(sim) = &game.sim else { return };
    if current.loaded != Some(current.number) || !sim.finished() || game.terrain.is_none() {
        return;
    }
    if let Some(back_to) = game.replay.as_ref().and_then(|r| r.demo) {
        current.loaded = None;
        w.next_state = Some(back_to);
        return;
    }
    let outcome = Outcome {
        level: current.number,
        saved: sim.counts.saved,
        // The level's lemmings: a nuke cuts `to_release` down to those
        // already out, which turned 79 needed of 80 into 79 of 1.
        total: game
            .terrain
            .as_ref()
            .map_or(sim.to_release, |(level, ..)| level.lemmings as u32)
            .max(1),
        needed: game.save_requirement,
    };
    w.sfx.play(if outcome.passed() {
        "VOXFX/CHEERS1"
    } else {
        "VOXFX/BOOS"
    });
    let seconds_left = sim.time_left / l3d_sim::TICKS_PER_SECOND;
    let settings = &mut w.settings;
    if settings.record(outcome.level, outcome.saved, seconds_left, outcome.passed()) {
        settings.save();
    }
    w.results.outcome = Some(outcome);
    w.next_state = Some(AppState::Results);
}

/// Leaving the results takes their screen away.
pub(crate) fn exit(w: &mut World) {
    w.ui.remove_opt(&mut w.results.root);
    w.results.choices.clear();
}

pub(crate) fn spawn_results(w: &mut World) {
    let Some(o) = w.results.outcome else { return };
    if w.results.root.is_some() {
        return;
    }
    let Ok(level) = w.data.0.level(o.level) else {
        return;
    };
    let pics = crate::briefing::load_scene(&mut w.data, level.theme, &mut w.e.images);
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
    w.results.root = Some(root);
    let canvas = ui.child(root, UiNode::new().canvas(SCREEN));
    if let Some(p) = &pics {
        ui.child(
            canvas,
            UiNode::image(p.picture.clone()).at(0.0, 0.0, SCREEN.x, SCREEN.y),
        );
        ui.child(
            canvas,
            UiNode::new()
                .at(120.0, 120.0, 400.0, 200.0)
                .background(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        );
        for (i, x) in [(0, 25.0), (1, 522.0)] {
            if let Some(h) = p.prompts.get(i).cloned() {
                ui.child(canvas, UiNode::image(h).at(x, 438.0, 32.0, 32.0));
            }
        }
    }
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let mut text = |s: &str, x: f32, y: f32, centred: bool| {
        let (glyphs, width) = font.layout(s);
        let left = if centred {
            x - width * TEXT_SCALE / 2.0
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
    text(
        &format!("Level {}  {}", o.level + 1, level.title.trim()),
        21.0,
        6.0,
        false,
    );
    let verdict = match (o.passed(), o.saved == o.total) {
        (true, true) => "Superb! Everyone saved!",
        (true, false) => "Well done!",
        (false, _) => "Not enough lemmings saved",
    };
    text(verdict, 320.0, 140.0, true);
    text(
        &format!("You rescued {} of {}", o.saved, o.total),
        320.0,
        180.0,
        true,
    );
    text(&format!("You needed {}", o.needed), 320.0, 210.0, true);
    let next_level = if o.passed() {
        (o.level + 1) % LEVELS
    } else {
        o.level
    };
    if o.passed()
        && let Some(code) = crate::codes::code_for_level(next_level)
    {
        text(&format!("Password :- {code}"), 320.0, 260.0, true);
    }
    text(
        if o.passed() { "Next Level" } else { "Retry" },
        61.0,
        445.0,
        false,
    );
    text("Menu", 561.0, 445.0, false);
    for (choice, x, width) in [(Choice::Next, 25.0, 200.0), (Choice::Menu, 522.0, 110.0)] {
        let id =
            w.ui.child(canvas, UiNode::new().button().at(x, 438.0, width, 32.0));
        w.results.choices.push((id, choice));
    }
}

/// Left click, Space or Enter: the next level (or a retry); right click or
/// Esc: the level list.
pub(crate) fn results_input(w: &mut World) {
    let Some(o) = w.results.outcome else { return };
    let lifted = crate::touch::lifted(w);
    let (ui, choices) = (&w.ui, &w.results.choices);
    let on = |c: Choice| {
        choices
            .iter()
            .any(|&(id, k)| ui.interaction(id) == Interaction::Pressed && k == c)
    };
    let on_prompt = choices
        .iter()
        .any(|&(id, _)| ui.interaction(id) != Interaction::None);
    let (keys, mouse) = (&w.e.input.keys, &w.e.input.mouse);
    let menu = mouse.just_pressed(MouseButton::Right)
        || keys.just_pressed(KeyCode::Escape)
        || on(Choice::Menu);
    let go = on(Choice::Next)
        || (mouse.just_pressed(MouseButton::Left) && !on_prompt)
        || (lifted && !on_prompt)
        || keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::NumpadEnter]);
    if menu {
        w.next_state = Some(crate::menu::level_menu(o.level));
    } else if go {
        w.current.number = if o.passed() {
            (o.level + 1) % LEVELS
        } else {
            o.level
        };
        w.current.loaded = None;
        w.next_state = Some(AppState::Briefing);
    }
}
