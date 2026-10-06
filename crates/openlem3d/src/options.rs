//! The configuration screen, laid out like the original's (a capture of it
//! is described in `docs/spec/ui-graphics.md`): large-font labels in boxes,
//! toggles shown as a yellow (on) or red (off) light, sliders as ten
//! lights, "Default Config" and "Exit and Save" along the bottom. Only
//! settings that mean something here are offered. Esc also saves and
//! leaves.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use l3d_formats::icons::Icons;

use crate::Data;
use crate::menu::AppState;
use crate::settings::{SLIDER_STEPS, Settings};
use crate::title::{Art, ScreenRoot, at, image_at, spawn_screen};

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load())
            .init_resource::<OptionsFrom>()
            .add_systems(Update, open_in_level.run_if(in_state(AppState::Playing)))
            .add_systems(Startup, (load_lights, apply_window).chain())
            .add_systems(Update, (spawn_options, options_input, show_lights).chain().run_if(in_state(AppState::Options)))
            .add_systems(Update, apply_window.run_if(resource_changed::<Settings>));
    }
}

/// The yellow and red lights (the small font's codes 35 and 36), and the
/// small font the labels use (their size on a capture of the original).
#[derive(Resource)]
struct Lights {
    on: Handle<Image>,
    off: Handle<Image>,
    font: crate::title::Glyphs,
}

fn load_lights(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let (Ok(pal), Some(icons)) = (d.palette("GFX/LM3D.PAL"), d.read("GFX/ICONS.RNC").ok().and_then(|raw| Icons::parse(&raw).ok())) else { return };
    let mut add = |c: u8| {
        let g = icons.small_font.glyph(c)?;
        let mut image = Image::new(
            Extent3d { width: g.width as u32, height: g.height as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            g.to_rgba(&pal, true),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        Some(images.add(image))
    };
    if let (Some(on), Some(off)) = (add(b'#'), add(b'$')) {
        let font = crate::title::glyphs(&icons.small_font, &pal, 4.0, 1.0, &mut images);
        commands.insert_resource(Lights { on, off, font });
    }
}

/// A setting on the screen.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Item {
    Land,
    Sea,
    Sky,
    LeftHanded,
    Fullscreen,
    Music,
    Effects,
    Camera,
}

/// A clickable part of the screen.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Target {
    Toggle(Item),
    /// A slider's light, 1-based.
    Light(Item, u8),
    Default,
    Exit,
}

/// A light showing a toggle or one step of a slider.
#[derive(Component, Clone, Copy)]
struct Light(Item, u8);

/// Box columns and rows, in the original's 320×200 pixels (measured on a
/// capture of its configuration screen).
const COLUMNS: [f32; 4] = [0.0, 80.0, 160.0, 240.0];
const TOGGLE_ROWS: [f32; 2] = [37.0, 69.0];
const SLIDER_ROWS: [f32; 2] = [101.0, 117.0];
const SLIDER_COLUMNS: [(f32, f32); 2] = [(0.0, 64.0), (160.0, 224.0)];
const BOX_HEIGHT: f32 = 14.0;
const TEXT_SCALE: f32 = 1.0;

const TOGGLES: [(Item, &str, usize, usize); 5] = [
    (Item::Land, "Land", 0, 0),
    (Item::Sea, "Sea", 0, 1),
    (Item::Sky, "Sky", 0, 2),
    (Item::LeftHanded, "Left Handed", 0, 3),
    (Item::Fullscreen, "Fullscreen", 1, 0),
];
const SLIDERS: [(Item, &str, usize, usize); 3] =
    [(Item::Music, "CD Music", 0, 0), (Item::Effects, "Effects", 1, 0), (Item::Camera, "Camera", 0, 1)];

fn panel_box(commands: &mut Commands, parent: Entity, x: f32, y: f32, w: f32, h: f32) -> Entity {
    let e = commands
        .spawn((at(x, y, w, h), Node { position_type: PositionType::Absolute, border: UiRect::all(px(1)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.02, 0.2, 0.7)), BorderColor::all(Color::srgb(0.25, 0.35, 0.8))))
        .id();
    commands.entity(parent).add_child(e);
    e
}

fn spawn_options(mut commands: Commands, art: Option<Res<Art>>, lights: Option<Res<Lights>>, roots: Query<(), With<ScreenRoot>>) {
    let (Some(art), Some(lights)) = (art, lights) else { return };
    if !roots.is_empty() {
        return;
    }
    let canvas = spawn_screen(&mut commands, &art, 0.0, Color::LinearRgba(LinearRgba::new(0.5, 0.9, 3.0, 1.0)));
    let font = &lights.font;
    let text = |commands: &mut Commands, s: &str, x: f32, y: f32| {
        let (glyphs, _) = font.layout(s);
        for (gx, image) in glyphs {
            let e = commands.spawn(image_at(image, at(x + gx * TEXT_SCALE, y, font.size.x * TEXT_SCALE, font.size.y * TEXT_SCALE))).id();
            commands.entity(canvas).add_child(e);
        }
    };
    let centred = |commands: &mut Commands, s: &str, cx: f32, y: f32| {
        let (_, w) = font.layout(s);
        text(commands, s, (cx - w * TEXT_SCALE / 2.0).round(), y);
    };
    panel_box(&mut commands, canvas, 112.0, 4.0, 96.0, 16.0);
    centred(&mut commands, "CONFIGURATION", 160.0, 8.0);
    for (item, label, row, col) in TOGGLES {
        let (x, y) = (COLUMNS[col], TOGGLE_ROWS[row]);
        let b = panel_box(&mut commands, canvas, x, y, 80.0, BOX_HEIGHT);
        commands.entity(b).insert((Button, Target::Toggle(item)));
        text(&mut commands, &label.to_uppercase(), x + 5.0, y + 3.0);
        let l = commands.spawn((Light(item, 1), image_at(lights.off.clone(), at(x + 67.0, y + 3.0, 7.0, 8.0)), Pickable::IGNORE)).id();
        commands.entity(canvas).add_child(l);
    }
    for (item, label, row, col) in SLIDERS {
        let ((lx, x0), y) = (SLIDER_COLUMNS[col], SLIDER_ROWS[row]);
        panel_box(&mut commands, canvas, lx, y, 160.0, BOX_HEIGHT);
        text(&mut commands, &label.to_uppercase(), lx + 5.0, y + 3.0);
        for i in 1..=SLIDER_STEPS {
            let r = at(x0 + 8.0 * (i - 1) as f32, y + 3.0, 7.0, 8.0);
            let l = commands.spawn((Light(item, i), Button, Target::Light(item, i), image_at(lights.off.clone(), r))).id();
            commands.entity(canvas).add_child(l);
        }
    }
    for (target, label, x) in [(Target::Default, "Default Config", 0.0), (Target::Exit, "Exit and Save", 240.0)] {
        let b = panel_box(&mut commands, canvas, x, 180.0, 80.0, 15.0);
        commands.entity(b).insert((Button, target));
        centred(&mut commands, &label.to_uppercase(), x + 40.0, 184.0);
    }
}

fn toggle(s: &mut Settings, item: Item) {
    match item {
        Item::Land => s.land = !s.land,
        Item::Sea => s.sea = !s.sea,
        Item::Sky => s.sky = !s.sky,
        Item::LeftHanded => s.left_handed = !s.left_handed,
        Item::Fullscreen => s.fullscreen = !s.fullscreen,
        _ => {}
    }
}

fn level(s: &mut Settings, item: Item) -> Option<&mut u8> {
    match item {
        Item::Music => Some(&mut s.music),
        Item::Effects => Some(&mut s.effects),
        Item::Camera => Some(&mut s.camera),
        _ => None,
    }
}

fn options_input(
    targets: Query<(&Interaction, &Target), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut from: ResMut<OptionsFrom>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (i, t) in &targets {
        if *i != Interaction::Pressed {
            continue;
        }
        match *t {
            Target::Toggle(item) => toggle(&mut settings, item),
            Target::Light(item, n) => {
                if let Some(v) = level(&mut settings, item) {
                    // Clicking the top light lit turns the slider off.
                    *v = if *v == n { n - 1 } else { n };
                }
            }
            Target::Default => *settings = Settings::default(),
            Target::Exit => {
                settings.save();
                next.set(std::mem::take(&mut from.0));
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        settings.save();
        next.set(std::mem::take(&mut from.0));
    }
}

fn show_lights(settings: Res<Settings>, lights: Option<Res<Lights>>, mut nodes: Query<(&Light, &mut ImageNode)>) {
    let Some(lights) = lights else { return };
    for (l, mut node) in &mut nodes {
        let mut s = settings.clone();
        let on = match l.0 {
            Item::Land => s.land,
            Item::Sea => s.sea,
            Item::Sky => s.sky,
            Item::LeftHanded => s.left_handed,
            Item::Fullscreen => s.fullscreen,
            item => level(&mut s, item).is_some_and(|v| *v >= l.1),
        };
        let h = if on { &lights.on } else { &lights.off };
        if node.image != *h {
            node.image = h.clone();
        }
    }
}

fn apply_window(settings: Res<Settings>, mut windows: Query<&mut Window>) {
    use bevy::window::{MonitorSelection, WindowMode};
    for mut w in &mut windows {
        let mode = if settings.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
        if w.mode != mode {
            w.mode = mode;
        }
    }
}

/// The screen the options screen returns to: the title screen, or the level
/// it was opened from (F12, as in the original).
#[derive(Resource, Default)]
pub struct OptionsFrom(pub AppState);

fn open_in_level(keys: Res<ButtonInput<KeyCode>>, mut from: ResMut<OptionsFrom>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::F12) {
        from.0 = AppState::Playing;
        next.set(AppState::Options);
    }
}
