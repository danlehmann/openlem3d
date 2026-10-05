//! Heads-up display: skill buttons, counters, and assigning skills by
//! clicking or tapping a lemming. Works with mouse, keyboard and touch.

use bevy::prelude::*;
use bevy::text::FontSize;
use l3d_sim::{SUB, Skill};

use crate::Game;
use crate::menu::AppState;
use crate::scene_render::SceneCamera;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedSkill>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (skill_buttons, skill_keys, update_labels, assign_on_pointer)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, show_in_play);
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

fn spawn_hud(mut commands: Commands) {
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
            for skill in Skill::ALL {
                bar.spawn((
                    Button,
                    SkillButton(skill),
                    Node {
                        width: px(84),
                        height: px(52),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(skill.name()),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextLayout::justify(Justify::Center),
                        SkillLabel(skill),
                    ));
                });
            }
        });
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
    for mut text in &mut status {
        let c = sim.counts;
        text.0 = format!(
            "Out {}   In {} / {}   Released {} / {}{}",
            sim.out(),
            c.saved,
            game.save_requirement,
            c.released,
            sim.to_release,
            if game.paused { "   PAUSED" } else { "" }
        );
    }
}

/// Screen-space radius, in logical pixels per unit of distance-scaled size,
/// within which a click selects a lemming.
const PICK_RADIUS_UNITS: f32 = 0.35;

/// Assigns the selected skill to the lemming nearest a click or tap.
fn assign_on_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    ui: Query<&Interaction>,
    camera: Res<SceneCamera>,
    selected: Res<SelectedSkill>,
    mut game: ResMut<Game>,
) {
    let Some(skill) = selected.0 else { return };
    let Ok(window) = windows.single() else { return };
    // Clicks on HUD buttons are not world clicks.
    if ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let point = if mouse.just_pressed(MouseButton::Left) {
        window.cursor_position()
    } else {
        touches.iter_just_pressed().next().map(|t| t.position())
    };
    let Some(point) = point else { return };
    let Some(sim) = &mut game.sim else { return };
    let size = Vec2::new(window.width(), window.height());
    let mut best: Option<(usize, f32)> = None;
    for (i, l) in sim.lemmings.iter().enumerate() {
        if l.gone {
            continue;
        }
        let centre = Vec3::from_array(l.pos.map(|v| v as f32 / SUB as f32)) + Vec3::Y * 0.2;
        let clip = camera.view_proj * centre.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        let screen = Vec2::new((ndc.x + 1.0) / 2.0 * size.x, (1.0 - ndc.y) / 2.0 * size.y);
        // Pixels per world unit at this depth, from the projection's y scale.
        let px_per_unit = camera.view_proj.y_axis.y.abs() / clip.w * size.y / 2.0;
        let d = screen.distance(point);
        if d < PICK_RADIUS_UNITS * px_per_unit.max(20.0) && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    if let Some((i, _)) = best {
        sim.assign(i, skill);
    }
}
