//! Fades from black between some screens, as in the original
//! (`docs/spec/ui-graphics.md`, "Screen transitions"). Only the fade-in is
//! kept: the new screen is up, and takes input, from its first frame.

use l3d_engine::prelude::*;

use crate::World;
use crate::menu::AppState;
use crate::ui::{Node, NodeId, UiNode, percent};

/// Fade-in durations, seconds (observed, 30 ms bursts): the code screen
/// fades in in about 0.15 s after a 0.6 s fade-out and 0.15 s of black (we
/// stretch the fade-in to stand for the whole change); the title fades in
/// in about 0.42 s after the code screen and 0.43 s after Options.
const CODE_IN: f32 = 0.3;
const TITLE_FROM_CODE_IN: f32 = 0.42;
const TITLE_FROM_OPTIONS_IN: f32 = 0.43;
/// From the briefing into the level or its preview (measured: the 3D view
/// fades in over about 0.4 s after the briefing has faded out).
const LEVEL_IN: f32 = 0.4;

/// The black overlay: when its fade started and how long it lasts.
#[derive(Default)]
pub(crate) struct Fade {
    node: Option<NodeId>,
    start: f32,
    duration: f32,
}

pub(crate) fn spawn(w: &mut World) {
    let node = w.ui.root(
        UiNode::new()
            .layout(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .background(Color::NONE)
            .z(i32::MAX - 1),
    );
    w.fade.node = Some(node);
}

/// Starts a fade for a change from `exited` to `entered`, if it has one.
pub(crate) fn transition(w: &mut World, exited: AppState, entered: AppState) {
    use AppState::*;
    let duration = match (exited, entered) {
        (Title, Code) => CODE_IN,
        (Code, Title) => TITLE_FROM_CODE_IN,
        (Options, Title) => TITLE_FROM_OPTIONS_IN,
        (Briefing, Playing) => LEVEL_IN,
        _ => return,
    };
    w.fade.start = w.e.time.game.elapsed_secs();
    w.fade.duration = duration;
}

pub(crate) fn fade(w: &mut World) {
    let f = &w.fade;
    let t = if f.duration > 0.0 {
        (w.e.time.game.elapsed_secs() - f.start) / f.duration
    } else {
        1.0
    };
    let colour = Color::BLACK.with_alpha((1.0 - t).clamp(0.0, 1.0));
    if let Some(n) = f.node.and_then(|n| w.ui.get_mut(n)) {
        n.background = Some(colour);
    }
}
