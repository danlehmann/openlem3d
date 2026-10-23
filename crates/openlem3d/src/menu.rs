//! Level selection: every level is available from the start, grouped by
//! difficulty rating (`docs/spec/level.md` for the file numbering).

use l3d_engine::input::MouseScrollUnit;
use l3d_engine::prelude::*;

use crate::World;
use crate::ui::{Interaction, Node, NodeId, Ui, UiNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// The title screen (main menu).
    #[default]
    Title,
    /// Entering a level code.
    Code,
    /// The configuration screen.
    Options,
    /// The Practice menu.
    Practice,
    /// Choosing a level.
    Menu,
    /// The level's briefing, before it starts.
    Briefing,
    Playing,
    /// The end-of-level screen.
    Results,
}

/// The difficulty ratings in file order; level `n` (1-based) of rating `r`
/// is `LEVEL.(20·r + n − 1)`.
pub const RATINGS: [&str; 5] = ["Fun", "Tricky", "Taxing", "Mayhem", "Practice"];
pub const LEVELS_PER_RATING: u32 = 20;
/// The Practice rating's index in [`RATINGS`].
pub const PRACTICE: usize = 4;

pub(crate) struct Titles(pub(crate) Vec<String>);

#[derive(Default)]
pub struct MenuRating(pub usize);

pub(crate) fn read_titles(w: &mut World) {
    let data = &mut w.data;
    let titles = (0..RATINGS.len() as u32 * LEVELS_PER_RATING)
        .map(|n| {
            data.0
                .level(n)
                .map(|l| l.title.trim().to_string())
                .unwrap_or_else(|_| format!("LEVEL.{n:03}"))
        })
        .collect();
    w.titles = Some(Titles(titles));
}

pub(crate) fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| {
                    f.to_uppercase()
                        .chain(c.flat_map(char::to_lowercase))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The level list, laid out like the original's "Select Fun Level To Play"
/// (from a capture; positions estimated, in 320×200): the title in white,
/// one red row per level ("1  Take A Dive"), and the best result under the
/// IN and clock icons on the right. The original lists only unlocked
/// levels; all twenty are open here, so the list scrolls (wheel, arrow
/// keys or Page Up/Down). Left and Right change the rating.
const ROW_TOP: f32 = 38.0;
const ROW_STEP: f32 = 15.0;
const ROWS_SHOWN: usize = 10;
const RED: Color = Color::srgb(1.0, 0.1, 0.1);
const HOVER: Color = Color::srgb(1.0, 0.9, 0.2);

/// How far the list is scrolled, in rows (a wheel, trackpad or finger
/// scrolls it smoothly, the keys by whole rows).
/// The level list: its labels' pictures, the scroll, the rows' buttons and
/// glyphs (by level), the scrolled body, and the row a finger landed on.
#[derive(Default)]
pub(crate) struct LevelList {
    labels: Option<(Handle<Image>, Handle<Image>)>,
    scroll: f32,
    rows: Vec<(NodeId, u32)>,
    glyphs: Vec<(NodeId, u32)>,
    body: Option<NodeId>,
    tapped: Option<u32>,
}

pub(crate) fn load_labels(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let (Ok(pal), Some(icons)) = (
        d.palette("GFX/LM3D.PAL"),
        d.read("GFX/ICONS.RNC")
            .ok()
            .and_then(|r| l3d_formats::icons::Icons::parse(&r).ok()),
    ) else {
        return;
    };
    let mut up = |img| images.add(crate::texture::indexed_image(img, &pal, true));
    if let (Some(inl), Some(clock)) = (icons.labels.first(), icons.labels.get(2)) {
        w.list.labels = Some((up(inl), up(clock)));
    }
}

pub(crate) fn ensure_menu(w: &mut World) {
    let (Some(art), Some(titles)) = (&w.art, &w.titles) else {
        return;
    };
    let Some(labels) = w.list.labels.clone() else {
        return;
    };
    if w.screen_ui.is_some() {
        return;
    }
    let Some(font) = art.large.as_ref() else {
        return;
    };
    let ui = &mut w.ui;
    let screen = crate::title::spawn_screen(ui, art, 0.0, Color::srgb(0.75, 0.75, 0.75));
    let canvas = screen.canvas;
    w.screen_ui = Some(screen);
    let mut list = std::mem::take(&mut w.list);
    list.rows.clear();
    list.glyphs.clear();
    let text = |ui: &mut Ui,
                list: &mut LevelList,
                parent: NodeId,
                s: &str,
                x: f32,
                y: f32,
                right: bool,
                colour: Color,
                row: Option<u32>| {
        let (glyphs, w) = font.layout(s);
        let left = if right { x - w } else { x };
        for (gx, image) in glyphs {
            let g = ui.child(
                parent,
                UiNode::image(image)
                    .tint(colour)
                    .at(left + gx, y, font.size.x, font.size.y),
            );
            if let Some(n) = row {
                list.glyphs.push((g, n));
            }
        }
    };
    let rating = w.rating.0;
    let heading = format!("Select {} Level To Play", RATINGS[rating]);
    let (_, hw) = font.layout(&heading);
    text(
        ui,
        &mut list,
        canvas,
        &heading,
        (160.0 - hw / 2.0).round(),
        4.0,
        false,
        Color::WHITE,
        None,
    );
    // Back to the title screen (Esc), for players without a keyboard.
    crate::key_buttons::spawn_text_button(
        ui,
        &mut w.key_buttons,
        canvas,
        font,
        "Back",
        4.0,
        4.0,
        RED,
        KeyCode::Escape,
    );
    for (img, x) in [(&labels.0, 246.0), (&labels.1, 287.0)] {
        ui.child(canvas, UiNode::image(img.clone()).at(x, 18.0, 32.0, 16.0));
    }
    // Rows clipped to the list area.
    let window = ui.child(
        canvas,
        UiNode::new()
            .at(0.0, ROW_TOP, 320.0, ROW_STEP * ROWS_SHOWN as f32)
            .layout(Node {
                clip: true,
                ..default()
            }),
    );
    let body = ui.child(
        window,
        UiNode::new().at(
            0.0,
            -list.scroll * ROW_STEP,
            320.0,
            ROW_STEP * LEVELS_PER_RATING as f32,
        ),
    );
    list.body = Some(body);
    let rating = rating as u32;
    let titles = &titles.0;
    for i in 0..LEVELS_PER_RATING {
        let n = rating * LEVELS_PER_RATING + i;
        let y = i as f32 * ROW_STEP;
        // An invisible button spanning the row.
        let hit = ui.child(body, UiNode::new().button().at(0.0, y, 320.0, ROW_STEP));
        list.rows.push((hit, n));
        text(
            ui,
            &mut list,
            body,
            &(i + 1).to_string(),
            22.0,
            y,
            true,
            RED,
            Some(n),
        );
        text(
            ui,
            &mut list,
            body,
            &title_case(&titles[n as usize]),
            32.0,
            y,
            false,
            RED,
            Some(n),
        );
        let (saved, secs) = w.settings.best.get(&n).copied().unwrap_or((0, 0));
        text(
            ui,
            &mut list,
            body,
            &saved.to_string(),
            270.0,
            y,
            true,
            RED,
            Some(n),
        );
        text(
            ui,
            &mut list,
            body,
            &format!("{}:{:02}", secs / 60, secs % 60),
            316.0,
            y,
            true,
            RED,
            Some(n),
        );
    }
    w.list = list;
}

/// Clicks, hover colour, scrolling and rating changes.
pub(crate) fn menu_input(w: &mut World) {
    let max = (LEVELS_PER_RATING as usize - ROWS_SHOWN) as f32;
    let (keys, touches) = (&w.e.input.keys, &w.e.input.touches);
    let screen = &w.screen;
    let list = &mut w.list;
    // Scrolling and dragging move the rows as far as the wheel or fingers
    // go (a wheel notch a row).
    let row_px = ROW_STEP * (screen.size.x / 320.0).min(screen.size.y / 200.0);
    let scale = w.e.window.scale_factor();
    let mut delta: f32 =
        w.e.input
            .wheel
            .iter()
            .map(|w| match w.unit {
                MouseScrollUnit::Line => -w.y,
                MouseScrollUnit::Pixel => -w.y / scale / row_px,
            })
            .sum();
    delta -= touches.iter().map(|t| t.delta().y).sum::<f32>() / row_px;
    // The keys step to whole rows.
    let steps = keys.just_pressed(KeyCode::ArrowDown) as i32
        - keys.just_pressed(KeyCode::ArrowUp) as i32
        + (keys.just_pressed(KeyCode::PageDown) as i32 - keys.just_pressed(KeyCode::PageUp) as i32)
            * ROWS_SHOWN as i32;
    if steps != 0 {
        delta += (list.scroll + steps as f32).round() - list.scroll;
    }
    if delta != 0.0 {
        list.scroll = (list.scroll + delta).clamp(0.0, max);
        if let Some(body) = list.body.and_then(|b| w.ui.get_mut(b)) {
            body.at = Some(Rect::new(
                0.0,
                -list.scroll * ROW_STEP,
                320.0,
                -list.scroll * ROW_STEP + ROW_STEP * LEVELS_PER_RATING as f32,
            ));
        }
    }
    // Left and Right step through the ratings (Practice has its own menu).
    let step = keys.just_pressed(KeyCode::ArrowRight) as i32
        - keys.just_pressed(KeyCode::ArrowLeft) as i32;
    if step != 0 {
        w.rating.0 = (w.rating.0 as i32 + step).rem_euclid(PRACTICE as i32) as usize;
        list.scroll = 0.0;
        crate::title::close_screen(&mut w.ui, &mut w.screen_ui);
        return;
    }
    let ui = &mut w.ui;
    let hovered = list
        .rows
        .iter()
        .find(|&&(id, _)| ui.interaction(id) != Interaction::None)
        .map(|&(_, r)| r);
    for &(id, row) in &list.glyphs {
        let colour = if Some(row) == hovered { HOVER } else { RED };
        if let Some(n) = ui.get_mut(id) {
            n.set_tint(colour);
        }
    }
    // A finger chooses a row by tapping it (lifting without having dragged)
    // rather than by landing on it, so that it can drag the list; the mouse
    // chooses on the click.
    let pressed = list
        .rows
        .iter()
        .find(|&&(id, _)| ui.pressed(id))
        .map(|&(_, r)| r);
    let tapped = &mut list.tapped;
    let chosen = if touches.iter().next().is_some() || touches.any_just_released() {
        if touches.any_just_pressed() {
            *tapped = pressed;
        }
        let dragged = touches
            .iter()
            .chain(touches.iter_just_released())
            .any(|t| t.distance().length() >= crate::touch::TAP_SLOP);
        if dragged {
            *tapped = None;
        }
        if touches.any_just_released() {
            tapped.take()
        } else {
            None
        }
    } else {
        pressed
    };
    if let Some(row) = chosen {
        w.current.number = row;
        w.current.loaded = None;
        w.next_state = Some(AppState::Briefing);
    }
}

pub(crate) fn back_to_menu(w: &mut World) {
    let keys = &w.e.input.keys;
    let (confirm, current, game) = (&mut w.confirm, &mut w.current, &mut w.game);
    let restart = |game: &mut crate::Game, current: &mut crate::CurrentLevel| {
        if let Some(sim) = &game.sim {
            game.replay = Some(crate::Replay::new(sim.log.clone()));
            current.loaded = None;
        }
    };
    if let Some(was_paused) = confirm.question {
        if keys.any_just_pressed([KeyCode::KeyY, KeyCode::Enter, KeyCode::NumpadEnter]) {
            confirm.question = None;
            restart(game, current);
        } else if keys.any_just_pressed([KeyCode::KeyN, KeyCode::Escape]) {
            confirm.question = None;
            game.paused = was_paused;
        } else {
            game.paused = true;
        }
        return;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if game.replay.is_some() {
        w.next_state = Some(level_menu(current.number));
    } else if w.settings.enhanced && game.sim.is_some() {
        confirm.question = Some(game.paused);
        game.paused = true;
    } else {
        restart(game, current);
    }
}

pub fn level_menu(level: u32) -> AppState {
    if level >= PRACTICE as u32 * LEVELS_PER_RATING {
        AppState::Practice
    } else {
        AppState::Menu
    }
}

/// Esc on the level-select screen returns to the title screen.
pub(crate) fn menu_to_title(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::Escape) {
        w.next_state = Some(AppState::Title);
    }
}
