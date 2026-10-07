//! Fades from black between some screens, as in the original
//! (`docs/spec/ui-graphics.md`, "Screen transitions"). Only the fade-in is
//! kept: the new screen is up, and takes input, from its first frame.

use bevy::prelude::*;

use crate::menu::AppState;

pub struct FadePlugin;

impl Plugin for FadePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(OnTransition { exited: AppState::Title, entered: AppState::Code }, start(CODE_IN))
            .add_systems(OnTransition { exited: AppState::Code, entered: AppState::Title }, start(TITLE_FROM_CODE_IN))
            .add_systems(OnTransition { exited: AppState::Options, entered: AppState::Title }, start(TITLE_FROM_OPTIONS_IN))
            .add_systems(OnTransition { exited: AppState::Briefing, entered: AppState::Playing }, start(LEVEL_IN))
            .add_systems(Update, fade);
    }
}

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
#[derive(Component, Default)]
struct Fade {
    start: f32,
    duration: f32,
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Fade::default(),
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(i32::MAX - 1),
        Pickable::IGNORE,
    ));
}

fn start(duration: f32) -> impl Fn(Res<Time>, Query<&mut Fade>) {
    move |time, mut fades| {
        for mut f in &mut fades {
            *f = Fade { start: time.elapsed_secs(), duration };
        }
    }
}

fn fade(time: Res<Time>, mut fades: Query<(&Fade, &mut BackgroundColor)>) {
    for (f, mut bg) in &mut fades {
        let t = if f.duration > 0.0 { (time.elapsed_secs() - f.start) / f.duration } else { 1.0 };
        let colour = Color::BLACK.with_alpha((1.0 - t).clamp(0.0, 1.0));
        if bg.0 != colour {
            bg.0 = colour;
        }
    }
}
