//! The presentation mode: Original (the game as it was) or Enhanced (ours:
//! smooth camera moves, solid brick stairs). A label at the top right of the
//! title screen shows it and switches it when clicked; Tab switches it on
//! the title and during a level, where the new mode is named briefly.

use bevy::prelude::*;

use crate::menu::AppState;
use crate::settings::Settings;
use crate::title::{Art, At, Canvas, ScreenRoot, at, image_at};

pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (toggle_on_title, title_label).chain().run_if(in_state(AppState::Title)))
            .add_systems(Update, toggle_in_level.run_if(in_state(AppState::Playing)))
            .add_systems(OnExit(AppState::Playing), crate::title::despawn::<NoticeRoot>);
    }
}

/// The title canvas, in the original's pixels.
const TITLE_SCREEN: Vec2 = Vec2::new(320.0, 200.0);
/// The notice's canvas: the briefing's, shared with the preview overlay.
const NOTICE_SCREEN: Vec2 = Vec2::new(640.0, 480.0);
const NOTICE_SCALE: f32 = 3.0;
const NOTICE_Y: f32 = 60.0;
/// How long the notice shows, in seconds.
const NOTICE_SECONDS: f32 = 1.5;

fn name(enhanced: bool) -> &'static str {
    if enhanced { "Enhanced" } else { "Original" }
}

fn switch(settings: &mut Settings) {
    settings.enhanced = !settings.enhanced;
    settings.save();
}

/// The title's mode label: a button whose children are the glyphs.
#[derive(Component)]
struct ModeLabel;

fn toggle_on_title(keys: Res<ButtonInput<KeyCode>>, labels: Query<&Interaction, (With<ModeLabel>, Changed<Interaction>)>, mut settings: ResMut<Settings>) {
    if keys.just_pressed(KeyCode::Tab) || labels.iter().any(|i| *i == Interaction::Pressed) {
        switch(&mut settings);
    }
}

/// Builds the label once the title exists, and again when the mode changes.
fn title_label(
    mut commands: Commands,
    art: Option<Res<Art>>,
    settings: Res<Settings>,
    canvases: Query<(Entity, &Canvas), With<ChildOf>>,
    roots: Query<(), With<ScreenRoot>>,
    labels: Query<Entity, With<ModeLabel>>,
) {
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else { return };
    if roots.is_empty() || (!labels.is_empty() && !settings.is_changed()) {
        return;
    }
    let Some((canvas, _)) = canvases.iter().find(|(_, c)| c.0 == TITLE_SCREEN) else { return };
    for e in &labels {
        commands.entity(e).despawn();
    }
    let (glyphs, width) = font.layout(&format!("Mode: {}", name(settings.enhanced)));
    let left = TITLE_SCREEN.x - 2.0 - width;
    let label = commands.spawn((ModeLabel, Button, at(left, 0.0, width, font.size.y), Node { position_type: PositionType::Absolute, ..default() })).id();
    commands.entity(canvas).add_child(label);
    for (x, image) in glyphs {
        let g = commands.spawn((image_at(image, at(x, 0.0, font.size.x, font.size.y)), Pickable::IGNORE)).id();
        commands.entity(label).add_child(g);
    }
}

/// The notice naming the mode after Tab, and when it was switched.
#[derive(Component)]
struct NoticeRoot(f32);

fn toggle_in_level(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    art: Option<Res<Art>>,
    mut settings: ResMut<Settings>,
    roots: Query<(Entity, &NoticeRoot)>,
) {
    let now = time.elapsed_secs();
    let changed = keys.just_pressed(KeyCode::Tab);
    if changed {
        switch(&mut settings);
    }
    for (e, r) in &roots {
        if changed || now - r.0 > NOTICE_SECONDS {
            commands.entity(e).despawn();
        }
    }
    if !changed {
        return;
    }
    let Some(font) = art.as_ref().and_then(|a| a.large.as_ref()) else { return };
    let root = commands
        .spawn((NoticeRoot(now), Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE))
        .id();
    let canvas = commands.spawn((Canvas(NOTICE_SCREEN), Node { position_type: PositionType::Absolute, ..default() }, Pickable::IGNORE)).id();
    commands.entity(root).add_child(canvas);
    let (glyphs, width) = font.layout(name(settings.enhanced));
    let left = (NOTICE_SCREEN.x - width * NOTICE_SCALE) / 2.0;
    // A dark shadow, then the text.
    for (shade, d) in [(Some(Color::srgba(0.0, 0.0, 0.0, 0.7)), 2.0), (None, 0.0)] {
        for (x, image) in &glyphs {
            let mut bundle: (ImageNode, At, Node) =
                image_at(image.clone(), at(left + x * NOTICE_SCALE + d, NOTICE_Y + d, font.size.x * NOTICE_SCALE, font.size.y * NOTICE_SCALE));
            if let Some(c) = shade {
                bundle.0.color = c;
            }
            let g = commands.spawn((bundle, Pickable::IGNORE)).id();
            commands.entity(canvas).add_child(g);
        }
    }
}
