//! The configuration screen, laid out like the original's (a capture of it
//! is described in `docs/spec/ui-graphics.md`): large-font labels in boxes,
//! toggles shown as a yellow (on) or red (off) light, sliders as ten
//! lights, "Default Config" and "Exit and Save" along the bottom. Only
//! settings that mean something here are offered, and one of ours, Glow.
//! Esc also saves and leaves.

use l3d_engine::prelude::*;
use l3d_formats::icons::Icons;

use crate::World;
use crate::menu::AppState;
use crate::settings::{SLIDER_STEPS, Settings};
use crate::texture::indexed_image;
use crate::ui::{NodeId, Ui, UiNode, px};

/// The lights' pictures and the small font, the screen's buttons and its
/// lights, and the fullscreen setting last applied.
#[derive(Default)]
pub(crate) struct OptionsScreen {
    lights: Option<Lights>,
    targets: Vec<(NodeId, Target)>,
    shown: Vec<(NodeId, Light)>,
    fullscreen: Option<bool>,
}

struct Lights {
    on: Handle<Image>,
    off: Handle<Image>,
    font: crate::title::Glyphs,
}

pub(crate) fn load_lights(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let (Ok(pal), Some(icons)) = (
        d.palette("GFX/LM3D.PAL"),
        d.read("GFX/ICONS.RNC")
            .ok()
            .and_then(|raw| Icons::parse(&raw).ok()),
    ) else {
        return;
    };
    let mut add = |c: u8| {
        let g = icons.small_font.glyph(c)?;
        Some(images.add(indexed_image(g, &pal, true)))
    };
    if let (Some(on), Some(off)) = (add(b'#'), add(b'$')) {
        let font = crate::title::glyphs(&icons.small_font, &pal, 4.0, 1.0, images);
        w.options.lights = Some(Lights { on, off, font });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Item {
    Land,
    Sea,
    Sky,
    LeftHanded,
    Fullscreen,
    Glow,
    Music,
    Effects,
    Camera,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Toggle(Item),
    Light(Item, u8),
    Default,
    Exit,
}

#[derive(Clone, Copy)]
struct Light(Item, u8);

const COLUMNS: [f32; 4] = [0.0, 80.0, 160.0, 240.0];
const TOGGLE_ROWS: [f32; 2] = [37.0, 69.0];
const SLIDER_ROWS: [f32; 2] = [101.0, 117.0];
const SLIDER_COLUMNS: [(f32, f32); 2] = [(0.0, 64.0), (160.0, 224.0)];
const BOX_HEIGHT: f32 = 14.0;
const TEXT_SCALE: f32 = 1.0;

const TOGGLES: [(Item, &str, usize, usize); 6] = [
    (Item::Land, "Land", 0, 0),
    (Item::Sea, "Sea", 0, 1),
    (Item::Sky, "Sky", 0, 2),
    (Item::LeftHanded, "Left Handed", 0, 3),
    (Item::Fullscreen, "Fullscreen", 1, 0),
    (Item::Glow, "Glow", 1, 1),
];
const SLIDERS: [(Item, &str, usize, usize); 3] = [
    (Item::Music, "CD Music", 0, 0),
    (Item::Effects, "Effects", 1, 0),
    (Item::Camera, "Camera", 0, 1),
];

fn panel_box(ui: &mut Ui, parent: NodeId, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    ui.child(
        parent,
        UiNode::new()
            .at(x, y, w, h)
            .background(Color::srgba(0.0, 0.02, 0.2, 0.7))
            .border(px(1), Color::srgb(0.25, 0.35, 0.8)),
    )
}

pub(crate) fn spawn_options(w: &mut World) {
    let (Some(art), Some(lights)) = (&w.art, &w.options.lights) else {
        return;
    };
    if w.screen_ui.is_some() {
        return;
    }
    let ui = &mut w.ui;
    let screen = crate::title::spawn_screen(
        ui,
        art,
        0.0,
        Color::LinearRgba(LinearRgba::new(0.5, 0.9, 3.0, 1.0)),
    );
    let canvas = screen.canvas;
    w.screen_ui = Some(screen);
    w.options.targets.clear();
    w.options.shown.clear();
    let font = &lights.font;
    let text = |ui: &mut Ui, s: &str, x: f32, y: f32| {
        let (glyphs, _) = font.layout(s);
        for (gx, image) in glyphs {
            ui.child(
                canvas,
                UiNode::image(image).at(
                    x + gx * TEXT_SCALE,
                    y,
                    font.size.x * TEXT_SCALE,
                    font.size.y * TEXT_SCALE,
                ),
            );
        }
    };
    let centred = |ui: &mut Ui, s: &str, cx: f32, y: f32| {
        let (_, w) = font.layout(s);
        text(ui, s, (cx - w * TEXT_SCALE / 2.0).round(), y);
    };
    panel_box(ui, canvas, 112.0, 4.0, 96.0, 16.0);
    centred(ui, "CONFIGURATION", 160.0, 8.0);
    for (item, label, row, col) in TOGGLES {
        let (x, y) = (COLUMNS[col], TOGGLE_ROWS[row]);
        let b = panel_box(ui, canvas, x, y, 80.0, BOX_HEIGHT);
        if let Some(n) = ui.get_mut(b) {
            *n = std::mem::take(n).button();
        }
        w.options.targets.push((b, Target::Toggle(item)));
        text(ui, &label.to_uppercase(), x + 5.0, y + 3.0);
        let l = ui.child(
            canvas,
            UiNode::image(lights.off.clone()).at(x + 67.0, y + 3.0, 7.0, 8.0),
        );
        w.options.shown.push((l, Light(item, 1)));
    }
    for (item, label, row, col) in SLIDERS {
        let ((lx, x0), y) = (SLIDER_COLUMNS[col], SLIDER_ROWS[row]);
        panel_box(ui, canvas, lx, y, 160.0, BOX_HEIGHT);
        text(ui, &label.to_uppercase(), lx + 5.0, y + 3.0);
        for i in 1..=SLIDER_STEPS {
            let l = ui.child(
                canvas,
                UiNode::image(lights.off.clone()).button().at(
                    x0 + 8.0 * (i - 1) as f32,
                    y + 3.0,
                    7.0,
                    8.0,
                ),
            );
            w.options.shown.push((l, Light(item, i)));
            w.options.targets.push((l, Target::Light(item, i)));
        }
    }
    for (target, label, x) in [
        (Target::Default, "Default Config", 0.0),
        (Target::Exit, "Exit and Save", 240.0),
    ] {
        let b = panel_box(ui, canvas, x, 180.0, 80.0, 15.0);
        if let Some(n) = ui.get_mut(b) {
            *n = std::mem::take(n).button();
        }
        w.options.targets.push((b, target));
        centred(ui, &label.to_uppercase(), x + 40.0, 184.0);
    }
}

fn toggle(s: &mut Settings, item: Item) {
    match item {
        Item::Land => s.land = !s.land,
        Item::Sea => s.sea = !s.sea,
        Item::Sky => s.sky = !s.sky,
        Item::LeftHanded => s.left_handed = !s.left_handed,
        Item::Fullscreen => s.fullscreen = !s.fullscreen,
        Item::Glow => s.glow = !s.glow,
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

pub(crate) fn options_input(w: &mut World) {
    let settings = &mut w.settings;
    for &(id, t) in &w.options.targets {
        if !w.ui.just_pressed(id) {
            continue;
        }
        match t {
            Target::Toggle(item) => toggle(settings, item),
            Target::Light(item, n) => {
                if let Some(v) = level(settings, item) {
                    // Clicking the top light lit turns the slider off.
                    *v = if *v == n { n - 1 } else { n };
                }
            }
            Target::Default => *settings = Settings::default(),
            Target::Exit => {
                settings.save();
                w.next_state = Some(std::mem::take(&mut w.options_from.0));
            }
        }
    }
    if w.e.input.keys.just_pressed(KeyCode::Escape) {
        settings.save();
        w.next_state = Some(std::mem::take(&mut w.options_from.0));
    }
}

pub(crate) fn show_lights(w: &mut World) {
    let Some(lights) = &w.options.lights else {
        return;
    };
    for &(id, l) in &w.options.shown {
        let mut s = w.settings.clone();
        let on = match l.0 {
            Item::Land => s.land,
            Item::Sea => s.sea,
            Item::Sky => s.sky,
            Item::LeftHanded => s.left_handed,
            Item::Fullscreen => s.fullscreen,
            Item::Glow => s.glow,
            item => level(&mut s, item).is_some_and(|v| *v >= l.1),
        };
        w.ui.set_image(id, if on { &lights.on } else { &lights.off });
    }
}

/// Puts the window into full screen or back as the setting says.
pub(crate) fn apply_window(w: &mut World) {
    use l3d_engine::window::{MonitorSelection, WindowMode};
    let fullscreen = w.settings.fullscreen;
    w.e.window.mode = if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    // Where the window mode does nothing (an Android desktop's window).
    if w.options.fullscreen != Some(fullscreen) {
        w.options.fullscreen = Some(fullscreen);
        crate::platform::window_fullscreen(fullscreen);
    }
}

#[derive(Default)]
pub struct OptionsFrom(pub AppState);

pub(crate) fn open_in_level(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::F12) {
        w.options_from.0 = AppState::Playing;
        w.next_state = Some(AppState::Options);
    }
}
