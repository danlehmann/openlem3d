//! The configuration screen, laid out like the original's (a capture of it
//! is described in `docs/spec/ui-graphics.md`): large-font labels in boxes,
//! toggles shown as a yellow (on) or red (off) light, sliders as ten
//! lights, "Default Config" and "Exit and Save" along the bottom. Only
//! settings that mean something here are offered, and some of ours: Rewind,
//! FPS and the mode, named in its box and switched by clicking it.
//! Esc, or a four-finger tap, also saves and leaves; F12 or a four-finger
//! tap opens it from the title or a level. A gamepad steps a frame from
//! setting to setting (ours): A switches a toggle or the mode and presses
//! the buttons, the bumpers move a slider down and up.

use l3d_engine::prelude::*;
use l3d_formats::icons::Icons;

use crate::World;
use crate::menu::AppState;
use crate::settings::{Graphics, Mode, Preset, SLIDER_STEPS, Settings};
use crate::texture::indexed_image;
use crate::ui::{NodeId, Ui, UiNode, px};

/// The lights' pictures and the small font, the screen's buttons and its
/// lights, the mode's name with the mode it shows, and the fullscreen
/// setting last applied.
#[derive(Default)]
pub(crate) struct OptionsScreen {
    lights: Option<Lights>,
    targets: Vec<(NodeId, Target)>,
    shown: Vec<(NodeId, Light)>,
    mode: Option<(NodeId, Mode)>,
    fullscreen: Option<bool>,
    /// The settings and buttons as a gamepad steps through them: each one's
    /// box (canvas pixels), the frame round the one it is on, and which.
    entries: Vec<(Rect, Entry)>,
    frame: Option<NodeId>,
    focus: Option<usize>,
    /// The screen is to be built again (a value it names changed), the
    /// gamepad's frame where it was.
    rebuild: bool,
}

#[derive(Clone, Copy)]
enum Entry {
    Toggle(Item),
    Slider(Item),
    Default,
    Exit,
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
    Fullscreen,
    Rewind,
    Fps,
    Mode,
    Music,
    Effects,
    Camera,
    /// Enhanced's graphics (ours).
    Preset(Preset),
    Resolution,
    Glow,
    Waves,
    Reflections,
    Shadows,
    Filtering,
    Snow,
    Sandstorm,
    Crt,
    Windows,
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
const TOGGLE_ROWS: [f32; 1] = [28.0];
const SLIDER_ROWS: [f32; 2] = [44.0, 60.0];
const SLIDER_COLUMNS: [(f32, f32); 2] = [(0.0, 64.0), (160.0, 224.0)];
const BOX_HEIGHT: f32 = 14.0;
const TEXT_SCALE: f32 = 1.0;

const TOGGLES: [(Item, &str, usize, usize); 3] = [
    (Item::Fullscreen, "Fullscreen", 0, 0),
    (Item::Rewind, "Rewind", 0, 1),
    (Item::Fps, "FPS", 0, 2),
];
/// The mode's box, in the slider rows' free place, and where its name
/// starts in it.
const MODE_BOX: (f32, f32) = (160.0, 60.0);
const MODE_NAME_X: f32 = 224.0;
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
    w.options.entries = entries();
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
    // A box that is a button for `item`, with its label (and, for a
    // setting with a value, the value at `value_x`).
    let button = |ui: &mut Ui, item: Target, x: f32, y: f32, w_: f32, label: &str| {
        let b = panel_box(ui, canvas, x, y, w_, BOX_HEIGHT);
        if let Some(n) = ui.get_mut(b) {
            *n = std::mem::take(n).button();
        }
        text(ui, &label.to_uppercase(), x + 5.0, y + 3.0);
        (b, item)
    };
    // Enhanced's graphics (ours), under their heading.
    {
        panel_box(ui, canvas, 112.0, GRAPHICS_HEADING, 96.0, BOX_HEIGHT);
        centred(ui, "GRAPHICS", 160.0, GRAPHICS_HEADING + 3.0);
        let g = w.settings.graphics;
        let rows = w.e.window.resolution.actual_height();
        for (rect, entry, label) in graphics_layout(g, rows) {
            let Entry::Toggle(item) = entry else { continue };
            let (x, y, wd) = (rect.min.x, rect.min.y, rect.width());
            let target = button(ui, Target::Toggle(item), x, y, wd, &label);
            w.options.targets.push(target);
            if lit(&w.settings, item).is_some() {
                let l = ui.child(
                    canvas,
                    UiNode::image(lights.off.clone()).at(x + wd - 13.0, y + 3.0, 7.0, 8.0),
                );
                w.options.shown.push((l, Light(item, 1)));
            }
        }
        // Custom, when no preset matches: shown, not pressed.
        let (x, y) = (COLUMNS[3], PRESET_ROW);
        panel_box(ui, canvas, x, y, 80.0, BOX_HEIGHT);
        text(ui, "CUSTOM", x + 5.0, y + 3.0);
        let l = ui.child(
            canvas,
            UiNode::image(lights.off.clone()).at(x + 67.0, y + 3.0, 7.0, 8.0),
        );
        w.options
            .shown
            .push((l, Light(Item::Preset(Preset::Custom), 1)));
    }
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
    let (x, y) = MODE_BOX;
    let b = panel_box(ui, canvas, x, y, 160.0, BOX_HEIGHT);
    if let Some(n) = ui.get_mut(b) {
        *n = std::mem::take(n).button();
    }
    w.options.targets.push((b, Target::Toggle(Item::Mode)));
    text(ui, "MODE", x + 5.0, y + 3.0);
    w.options.mode = None;
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
    w.options.frame = Some(
        ui.child(
            canvas,
            UiNode::new()
                .at(0.0, 0.0, 1.0, 1.0)
                .border(px(2), Color::srgb(1.0, 0.9, 0.2))
                .hidden(),
        ),
    );
    place_frame(w);
}

/// The settings and buttons a gamepad steps through, with their boxes, as
/// `spawn_options` lays them out.
fn entries() -> Vec<(Rect, Entry)> {
    let mut e = Vec::new();
    for (item, _, row, col) in TOGGLES {
        let (x, y) = (COLUMNS[col], TOGGLE_ROWS[row]);
        e.push((
            Rect::new(x, y, x + 80.0, y + BOX_HEIGHT),
            Entry::Toggle(item),
        ));
    }
    for (item, _, row, col) in SLIDERS {
        let ((x, _), y) = (SLIDER_COLUMNS[col], SLIDER_ROWS[row]);
        e.push((
            Rect::new(x, y, x + 160.0, y + BOX_HEIGHT),
            Entry::Slider(item),
        ));
    }
    let (x, y) = MODE_BOX;
    e.push((
        Rect::new(x, y, x + 160.0, y + BOX_HEIGHT),
        Entry::Toggle(Item::Mode),
    ));
    e.push((Rect::new(0.0, 180.0, 80.0, 195.0), Entry::Default));
    e.push((Rect::new(240.0, 180.0, 320.0, 195.0), Entry::Exit));
    e.extend(
        graphics_layout(Graphics::default(), 1080)
            .into_iter()
            .map(|(r, e, _)| (r, e)),
    );
    e
}

/// The graphics' buttons (ours): their boxes, what they press and their
/// labels, for graphics `g` on a screen of `rows` rows. The presets in a
/// row (Custom shown beside them, `spawn_options`), the resolution and glow
/// with their values, the effects as lights.
fn graphics_layout(g: Graphics, rows: u32) -> Vec<(Rect, Entry, String)> {
    let mut e = Vec::new();
    let mut add = |x: f32, y: f32, w: f32, item: Item, label: String| {
        e.push((
            Rect::new(x, y, x + w, y + BOX_HEIGHT),
            Entry::Toggle(item),
            label,
        ));
    };
    for (i, p) in Preset::CHOICES.into_iter().enumerate() {
        add(
            COLUMNS[i],
            PRESET_ROW,
            80.0,
            Item::Preset(p),
            p.name().into(),
        );
    }
    let row = VALUE_ROW;
    add(
        0.0,
        row,
        160.0,
        Item::Resolution,
        format!(
            "Resolution {}",
            crate::settings::resolution_name(g.resolution, rows)
        ),
    );
    add(
        160.0,
        row,
        160.0,
        Item::Glow,
        format!("Glow {}", g.glow.name()),
    );
    let effects = [
        (Item::Waves, "Waves"),
        (Item::Reflections, "Reflections"),
        (Item::Shadows, "Shadows"),
        (Item::Filtering, "Filtering"),
        (Item::Snow, "Snow"),
        (Item::Sandstorm, "Sandstorm"),
        (Item::Crt, "CRT"),
        (Item::Windows, "Windows"),
    ];
    for (i, (item, label)) in effects.into_iter().enumerate() {
        let y = GRAPHICS_ROWS[i / 4];
        add(COLUMNS[i % 4], y, 80.0, item, label.into());
    }
    e
}

/// The graphics' heading and rows (ours), below the original's settings:
/// the presets, the resolution and glow, and two rows of effects.
const GRAPHICS_HEADING: f32 = 80.0;
const PRESET_ROW: f32 = 98.0;
const VALUE_ROW: f32 = 114.0;
const GRAPHICS_ROWS: [f32; 2] = [130.0, 146.0];

/// Whether `item`'s light is lit, for a setting shown with one (none for
/// the rest).
fn lit(s: &Settings, item: Item) -> Option<bool> {
    let g = s.graphics;
    Some(match item {
        Item::Fullscreen => s.fullscreen,
        Item::Rewind => s.rewind,
        Item::Fps => s.fps,
        Item::Preset(p) => g.preset == p,
        Item::Waves => g.waves,
        Item::Reflections => g.reflections,
        Item::Shadows => g.shadows,
        Item::Filtering => g.filtering,
        Item::Snow => g.snow,
        Item::Sandstorm => g.sandstorm,
        Item::Crt => g.crt,
        Item::Windows => g.windows,
        _ => return None,
    })
}

/// The box next to box `i` the way `way` points (its stronger axis): the
/// nearest past its edge that way, better the more it lines up with it
/// across, worse the further it lies off to the side. Left and right stay
/// in the row.
pub(crate) fn neighbour(boxes: &[Rect], i: usize, way: Vec2) -> Option<usize> {
    let from = boxes[i];
    // How much two intervals overlap; negative, how far apart they are.
    let overlap = |a0: f32, a1: f32, b0: f32, b1: f32| a1.min(b1) - a0.max(b0);
    let horizontal = way.x.abs() > way.y.abs();
    let forward = if horizontal { way.x } else { way.y } > 0.0;
    boxes
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .filter_map(|(j, r)| {
            let (gap, across) = match (horizontal, forward) {
                (true, true) => (
                    r.min.x - from.max.x,
                    overlap(from.min.y, from.max.y, r.min.y, r.max.y),
                ),
                (true, false) => (
                    from.min.x - r.max.x,
                    overlap(from.min.y, from.max.y, r.min.y, r.max.y),
                ),
                (false, true) => (
                    r.min.y - from.max.y,
                    overlap(from.min.x, from.max.x, r.min.x, r.max.x),
                ),
                (false, false) => (
                    from.min.y - r.max.y,
                    overlap(from.min.x, from.max.x, r.min.x, r.max.x),
                ),
            };
            if gap < -0.5 || (horizontal && across <= 0.0) {
                return None;
            }
            // Lined up counts the same however wide the overlap (a wide box
            // further on is no nearer); off to the side counts double.
            let side = if across > 0.0 {
                -across.min(ALIGNED_BONUS)
            } else {
                -across * 2.0
            };
            Some((j, gap + side))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(j, _)| j)
}

/// Puts the gamepad's frame round the setting it is on, if any (the screen
/// built again keeps it where it was).
fn place_frame(w: &mut World) {
    let o = &w.options;
    let Some((rect, _)) = o.focus.and_then(|i| o.entries.get(i)).copied() else {
        return;
    };
    if let Some(n) = o.frame.and_then(|f| w.ui.get_mut(f)) {
        n.at = Some(Rect::new(
            rect.min.x - 2.0,
            rect.min.y - 2.0,
            rect.max.x + 2.0,
            rect.max.y + 2.0,
        ));
        n.set_visible(true);
    }
}

/// How much being lined up with a box counts, in canvas pixels of distance.
const ALIGNED_BONUS: f32 = 4.0;

/// A gamepad on this screen: `way` the way the D-pad or stick stepped (or
/// zero), then A and the bumpers on the setting framed.
pub(crate) fn gamepad(w: &mut World, way: Vec2, pressed: bool, slide: i32) {
    let o = &mut w.options;
    if o.entries.is_empty() {
        return;
    }
    // With a gamepad connected, the frame starts on the first setting.
    if o.focus.is_none() {
        o.focus = Some(0);
    } else if let Some(i) = o.focus
        && way != Vec2::ZERO
    {
        let boxes: Vec<Rect> = o.entries.iter().map(|e| e.0).collect();
        o.focus = Some(neighbour(&boxes, i, way).unwrap_or(i));
    }
    let Some(i) = o.focus else {
        return;
    };
    let (rect, entry) = o.entries[i];
    if let Some(n) = o.frame.and_then(|f| w.ui.get_mut(f)) {
        n.at = Some(Rect::new(
            rect.min.x - 2.0,
            rect.min.y - 2.0,
            rect.max.x + 2.0,
            rect.max.y + 2.0,
        ));
        n.set_visible(true);
    }
    let stepped = matches!(
        entry,
        Entry::Toggle(Item::Mode | Item::Resolution | Item::Glow)
    );
    if let Entry::Toggle(item) = entry
        && (pressed || (stepped && slide != 0))
    {
        // The left bumper steps back, the right one (and A) on.
        if slide < 0
            && let Some(g) = graphics_by(w.settings.graphics, item, -1)
        {
            w.settings.graphics = g;
            w.options.rebuild = true;
        } else {
            press(w, item);
        }
        return;
    }
    let settings = &mut w.settings;
    match entry {
        Entry::Slider(item) if slide != 0 => {
            if let Some(v) = level(settings, item) {
                *v = (*v as i32 + slide).clamp(0, SLIDER_STEPS as i32) as u8;
            }
        }
        Entry::Default if pressed => *settings = Settings::default(),
        Entry::Exit if pressed => {
            settings.save();
            w.next_state = Some(std::mem::take(&mut w.options_from.0));
        }
        _ => {}
    }
}

fn toggle(s: &mut Settings, item: Item) {
    match item {
        Item::Fullscreen => s.fullscreen = !s.fullscreen,
        Item::Rewind => s.rewind = !s.rewind,
        Item::Fps => s.fps = !s.fps,
        Item::Mode => crate::style::switch(s),
        _ => {
            if let Some(g) = graphics(s.graphics, item) {
                s.graphics = g;
            }
        }
    }
}

/// Enhanced's graphics after pressing `item`: a preset's, or
/// one setting changed (then named by the preset it matches, or Custom).
fn graphics(g: Graphics, item: Item) -> Option<Graphics> {
    graphics_by(g, item, 1)
}

/// The same, a setting with several values stepped `step` (1 on, −1 back).
fn graphics_by(g: Graphics, item: Item, step: i32) -> Option<Graphics> {
    let mut next = match item {
        Item::Preset(p) => return Some(p.graphics(g)),
        // On is up: towards more rows (the steps run from the most down).
        Item::Resolution => Graphics {
            resolution: (g.resolution as i32 - step).rem_euclid(crate::settings::RESOLUTIONS as i32)
                as u8,
            ..g
        },
        Item::Glow => Graphics {
            glow: if step < 0 {
                g.glow.previous()
            } else {
                g.glow.next()
            },
            ..g
        },
        Item::Waves => Graphics {
            waves: !g.waves,
            ..g
        },
        Item::Reflections => Graphics {
            reflections: !g.reflections,
            ..g
        },
        Item::Shadows => Graphics {
            shadows: !g.shadows,
            ..g
        },
        Item::Filtering => Graphics {
            filtering: !g.filtering,
            ..g
        },
        Item::Snow => Graphics { snow: !g.snow, ..g },
        Item::Sandstorm => Graphics {
            sandstorm: !g.sandstorm,
            ..g
        },
        Item::Crt => Graphics { crt: !g.crt, ..g },
        Item::Windows => Graphics {
            windows: !g.windows,
            ..g
        },
        _ => return None,
    };
    next.preset = Preset::CHOICES
        .into_iter()
        .find(|p| Graphics { preset: *p, ..next } == p.graphics(next))
        .unwrap_or(Preset::Custom);
    Some(next)
}

/// Pressing `item` (with a click or a gamepad): settings change; the
/// screen is built again to name changed graphics values.
fn press(w: &mut World, item: Item) {
    toggle(&mut w.settings, item);
    if graphics(w.settings.graphics, item).is_some() {
        w.options.rebuild = true;
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
    let pressed: Vec<Item> = w
        .options
        .targets
        .iter()
        .filter(|&&(id, _)| w.ui.just_pressed(id))
        .filter_map(|&(_, t)| match t {
            Target::Toggle(item) => Some(item),
            _ => None,
        })
        .collect();
    for item in pressed {
        press(w, item);
    }
    let settings = &mut w.settings;
    for &(id, t) in &w.options.targets {
        if !w.ui.just_pressed(id) {
            continue;
        }
        match t {
            Target::Toggle(_) => {}
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
    let leave = w.e.input.keys.just_pressed(KeyCode::Escape) || crate::touch::four_finger_tap(w);
    if leave {
        let settings = &mut w.settings;
        settings.save();
        w.next_state = Some(std::mem::take(&mut w.options_from.0));
    }
    // Built again at once, not next frame: a frame without it flickered.
    if std::mem::take(&mut w.options.rebuild) && w.next_state.is_none() {
        crate::title::close_screen(&mut w.ui, &mut w.screen_ui);
        spawn_options(w);
    }
}

pub(crate) fn show_lights(w: &mut World) {
    let Some(lights) = &w.options.lights else {
        return;
    };
    for &(id, l) in &w.options.shown {
        let mut s = w.settings.clone();
        let on = lit(&s, l.0).unwrap_or_else(|| level(&mut s, l.0).is_some_and(|v| *v >= l.1));
        w.ui.set_image(id, if on { &lights.on } else { &lights.off });
    }
    show_mode(w);
}

/// Names the mode in its box, again when it changes.
fn show_mode(w: &mut World) {
    let (Some(lights), Some(screen)) = (&w.options.lights, &w.screen_ui) else {
        return;
    };
    let mode = w.settings.mode;
    if let Some((id, shown)) = w.options.mode {
        if shown == mode && w.ui.contains(id) {
            return;
        }
        w.ui.remove(id);
    }
    let font = &lights.font;
    let (glyphs, width) = font.layout(&mode.name().to_uppercase());
    let (_, y) = MODE_BOX;
    let name = w.ui.child(
        screen.canvas,
        UiNode::new().at(
            MODE_NAME_X,
            y + 3.0,
            width * TEXT_SCALE,
            font.size.y * TEXT_SCALE,
        ),
    );
    for (gx, image) in glyphs {
        w.ui.child(
            name,
            UiNode::image(image).at(
                gx * TEXT_SCALE,
                0.0,
                font.size.x * TEXT_SCALE,
                font.size.y * TEXT_SCALE,
            ),
        );
    }
    w.options.mode = Some((name, mode));
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

/// A four-finger tap on the title opens the configuration, as F12 does.
pub(crate) fn open_on_title(w: &mut World) {
    if crate::touch::four_finger_tap(w) {
        w.options_from.0 = AppState::Title;
        w.next_state = Some(AppState::Options);
    }
}

pub(crate) fn open_in_level(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::F12) || crate::touch::four_finger_tap(w) {
        w.options_from.0 = AppState::Playing;
        w.next_state = Some(AppState::Options);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the gamepad steps from each setting (`--nocapture` prints
    /// them all).
    #[test]
    fn gamepad_steps() {
        let e = entries();
        let name = |i: Option<usize>| match i.map(|i| e[i].1) {
            None => "-".to_string(),
            Some(Entry::Toggle(item) | Entry::Slider(item)) => format!("{item:?}"),
            Some(Entry::Default) => "Default".into(),
            Some(Entry::Exit) => "Exit".into(),
        };
        let boxes: Vec<Rect> = e.iter().map(|e| e.0).collect();
        for i in 0..e.len() {
            println!(
                "{:>10}: up {:>10} down {:>10} left {:>10} right {:>10}",
                name(Some(i)),
                name(neighbour(&boxes, i, Vec2::NEG_Y)),
                name(neighbour(&boxes, i, Vec2::Y)),
                name(neighbour(&boxes, i, Vec2::NEG_X)),
                name(neighbour(&boxes, i, Vec2::X)),
            );
        }
        let find = |n: &str| (0..e.len()).find(|&i| name(Some(i)) == n).unwrap();
        let step = |from: &str, way: Vec2| name(neighbour(&boxes, find(from), way));
        // Straight down the columns, not to the nearest middle (owner:
        // Fullscreen went down to Default Config).
        assert_eq!(step("Fullscreen", Vec2::Y), "Music");
        assert_eq!(step("Rewind", Vec2::Y), "Music");
        assert_eq!(step("Fps", Vec2::Y), "Camera");
        assert_eq!(step("Mode", Vec2::NEG_Y), "Camera");
        // Down into the graphics, and out of them to the buttons.
        assert_eq!(step("Mode", Vec2::Y), "Preset(High)");
        assert_eq!(step("Preset(Low)", Vec2::X), "Preset(Medium)");
        assert_eq!(step("Resolution", Vec2::NEG_Y), "Preset(Low)");
        assert_eq!(step("Snow", Vec2::Y), "Default");
        assert_eq!(step("Windows", Vec2::Y), "Exit");
        // Left and right stay in the row.
        assert_eq!(step("Fps", Vec2::X), "-");
        // Every setting and button can be reached from the first.
        let mut seen = vec![false; e.len()];
        let mut todo = vec![0];
        while let Some(i) = todo.pop() {
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            for way in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                todo.extend(neighbour(&boxes, i, way));
            }
        }
        let missed: Vec<_> = (0..e.len())
            .filter(|&i| !seen[i])
            .map(|i| name(Some(i)))
            .collect();
        assert!(missed.is_empty(), "unreachable: {missed:?}");
    }

    /// Changing one setting makes the preset Custom; changing it back, the
    /// preset again.
    #[test]
    fn custom_preset() {
        let high = Preset::High.graphics(Graphics::default());
        let changed = graphics(high, Item::Waves).unwrap();
        assert_eq!(changed.preset, Preset::Custom);
        assert_eq!(graphics(changed, Item::Waves).unwrap().preset, Preset::High);
        assert_eq!(
            graphics(high, Item::Preset(Preset::Low)).unwrap().preset,
            Preset::Low
        );
    }
}
