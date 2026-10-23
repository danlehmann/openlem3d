//! The Practice menu, laid out like the original's "Select Item to
//! Practice" (positions from a capture, estimated): a grid of twenty icons
//! for `LEVEL.080`–`099` plus EXIT. The skill icons are the panel's
//! `MINILEMM` lemmings; the others come from the panel icons, `DEFLICON`
//! and `PRACICON`. The hovered icon animates, its name shows at the bottom
//! and its voice plays (`SOUND/SAMPLIST.TXT`); a green tick marks levels
//! already completed.

use l3d_engine::prelude::*;
use l3d_formats::icons::{Icons, panel as icon};
use l3d_formats::image::IndexedImage;
use l3d_formats::sheets;

use crate::World;
use crate::menu::AppState;
use crate::texture::indexed_image;
use crate::ui::{Interaction, NodeId, UiNode};

#[derive(Clone, Copy)]
enum Source {
    /// A skill: its panel button's animation (index into `panel::SKILL_BUTTONS`).
    Skill(usize),
    /// Panel icons, played in turn.
    Panel(&'static [usize]),
    Deflicon,
    Prac(usize, usize),
}

/// The first Practice level.
const FIRST: u32 = 80;

/// The level an icon opens: the grid follows the level order, except that
/// the face (Virtual Lemming, level 90 "Claustrophobic") comes before the
/// arrow (Hi-Light Lemming, level 89), as in the original (hover labels and
/// briefings checked).
fn level_of(item: usize) -> u32 {
    FIRST
        + match item {
            9 => 10,
            10 => 9,
            i => i as u32,
        }
}
const ROW_Y: [f32; 3] = [37.0, 86.0, 136.0];
const COLUMNS: [[f32; 7]; 3] = [
    [22.0, 70.0, 115.0, 160.0, 200.0, 238.0, 277.0],
    [39.0, 78.0, 120.0, 160.0, 200.0, 238.0, 277.0],
    [36.0, 81.0, 118.0, 160.0, 205.0, 241.0, 281.0],
];
/// Icons in grid order: levels 80–99, then EXIT; each with its voice.
const ITEMS: [(Source, &str); 21] = [
    (Source::Skill(0), "VOXFX/BLOCKER1"),
    (Source::Skill(1), "VOXFX/TURNER1"),
    (Source::Skill(2), "VOXFX/BOMBER"),
    (Source::Skill(3), "VOXFX/BUILDER2"),
    (Source::Skill(4), "VOXFX/BASHER2"),
    (Source::Skill(5), "VOXFX/MINER2"),
    (Source::Skill(6), "VOXFX/DIGGER2"),
    (Source::Skill(7), "VOXFX/CLIMBER2"),
    (Source::Skill(8), "VOXFX/FLOATER1"),
    (
        Source::Panel(&[icon::FACE + 1, icon::FACE]),
        "VOXFX/VIRTUAL1",
    ),
    (Source::Panel(&[39, 40, 41, 42, 43, 44]), "VOXFX/HILITE"),
    (Source::Deflicon, "VOXFX/DEFLECT"),
    (Source::Prac(26, 8), "VOXFX/MUD1"),
    (Source::Prac(6, 4), "VOXFX/ONEWAY"),
    (Source::Prac(4, 2), "VOXFX/SPLITTER"),
    (Source::Prac(34, 5), "VOXFX/SLIPPER1"),
    (Source::Prac(10, 5), "VOXFX/ROCKSL1"),
    (Source::Prac(19, 7), "VOXFX/SPRING"),
    (Source::Prac(15, 4), "VOXFX/TRAMPOL1"),
    (Source::Prac(0, 4), "VOXFX/TELEPORT"),
    (Source::Prac(40, 16), "VOXFX/EXIT"),
];
const TICK: usize = 39;
const FPS: f32 = 12.0;

/// Uploaded frames per item, the tick and the umbrella.
/// The icons' pictures, the grid's icons and name line, and the item last
/// hovered.
#[derive(Default)]
pub(crate) struct Practice {
    pics: Option<Pics>,
    items: Vec<(NodeId, usize)>,
    line: Option<NodeId>,
    last: Option<usize>,
}

struct Pics {
    items: Vec<Vec<Handle<Image>>>,
    tick: Handle<Image>,
    umbrella: Option<Handle<Image>>,
}

pub(crate) fn load(w: &mut World) {
    let d = &mut w.data.0;
    let images = &mut w.e.images;
    let Ok(pal) = d.palette("GFX/LM3D.PAL") else {
        return;
    };
    let mut cut = |s: &sheets::Sheet| {
        d.read(s.path)
            .ok()
            .and_then(|raw| s.cut(&raw).ok())
            .unwrap_or_default()
    };
    let (mini, defl, prac) = (
        cut(&sheets::MINILEMM),
        cut(&sheets::DEFLICON),
        cut(&sheets::PRACICON),
    );
    let icons = d
        .read("GFX/ICONS.RNC")
        .ok()
        .and_then(|raw| Icons::parse(&raw).ok());
    let umbrella = icons.as_ref().map(|i| i.umbrella.clone());
    let panel = icons.map(|i| i.panel).unwrap_or_default();
    if mini.is_empty() || prac.len() <= TICK {
        return;
    }
    let mut up = |imgs: &[IndexedImage]| {
        imgs.iter()
            .map(|i| images.add(indexed_image(i, &pal, true)))
            .collect::<Vec<_>>()
    };
    let items = ITEMS
        .iter()
        .map(|(src, _)| match *src {
            Source::Skill(s) => up(&crate::panel::SKILL_BUTTONS[s]
                .1
                .iter()
                .filter_map(|&c| mini.get(c).cloned())
                .collect::<Vec<_>>()),
            Source::Panel(cells) => up(&cells
                .iter()
                .filter_map(|&c| panel.get(c).cloned())
                .collect::<Vec<_>>()),
            Source::Deflicon => up(&defl),
            Source::Prac(first, n) => up(&prac[first..first + n]),
        })
        .collect();
    let tick = up(&prac[TICK..=TICK]).remove(0);
    let umbrella = umbrella.map(|u| images.add(indexed_image(&u, &pal, true)));
    w.practice.pics = Some(Pics {
        items,
        tick,
        umbrella,
    });
}

pub(crate) fn spawn_grid(w: &mut World) {
    let (Some(art), Some(pics)) = (&w.art, &w.practice.pics) else {
        return;
    };
    if w.screen_ui.is_some() {
        return;
    }
    let ui = &mut w.ui;
    let screen = crate::title::spawn_screen(ui, art, 0.0, Color::srgb(0.75, 0.75, 0.75));
    let canvas = screen.canvas;
    w.screen_ui = Some(screen);
    if let Some(font) = &art.large {
        let (glyphs, w) = font.layout("Select Item to Practice");
        let left = (160.0 - w / 2.0).round();
        for (x, image) in glyphs {
            ui.child(
                canvas,
                UiNode::image(image).at(left + x, 12.0, font.size.x, font.size.y),
            );
        }
    }
    w.practice.items.clear();
    for (i, frames) in pics.items.iter().enumerate() {
        let (row, col) = (i / 7, i % 7);
        let (cx, cy) = (COLUMNS[row][col], ROW_Y[row] + 16.0);
        let size = if matches!(ITEMS[i].0, Source::Panel(_)) {
            24.0
        } else {
            32.0
        };
        let e = ui.child(
            canvas,
            UiNode::image(frames[0].clone()).button().at(
                cx - size / 2.0,
                cy - size / 2.0,
                size,
                size,
            ),
        );
        w.practice.items.push((e, i));
        // The floater holds its umbrella, a tile of its own above the icon,
        // placed as on the skill panel (owner's observation).
        if matches!(ITEMS[i].0, Source::Skill(8))
            && let Some(umbrella) = &pics.umbrella
        {
            ui.child(
                canvas,
                UiNode::image(umbrella.clone()).at(
                    cx - size / 2.0 - 2.0,
                    cy - size / 2.0 - 25.0,
                    32.0,
                    32.0,
                ),
            );
        }
        if i < 20 && w.settings.completed.contains(&level_of(i)) {
            ui.child(
                canvas,
                UiNode::image(pics.tick.clone()).at(cx - 4.0, cy - 16.0, 32.0, 32.0),
            );
        }
    }
    w.practice.line = Some(ui.child(canvas, UiNode::new().at(0.0, 0.0, 320.0, 200.0)));
}

pub(crate) fn hover(w: &mut World) {
    let (Some(pics), Some(art)) = (&w.practice.pics, &w.art) else {
        return;
    };
    let now = w.e.time.game.elapsed_secs();
    let mut hovered = None;
    for &(id, item) in &w.practice.items {
        let frames = &pics.items[item];
        let frame = if w.ui.interaction(id) != Interaction::None {
            hovered = Some(item);
            (now * FPS) as usize % frames.len()
        } else {
            0
        };
        w.ui.set_image(id, &frames[frame]);
    }
    if hovered == w.practice.last {
        return;
    }
    w.practice.last = hovered;
    let Some(line) = w.practice.line.filter(|&l| w.ui.contains(l)) else {
        return;
    };
    for c in w.ui.children(line).to_vec() {
        w.ui.remove(c);
    }
    let Some(i) = hovered else { return };
    w.sfx.play(ITEMS[i].1);
    let name = if i < 20 {
        w.titles
            .as_ref()
            .map(|t| crate::menu::title_case(&t.0[level_of(i) as usize]))
            .unwrap_or_default()
    } else {
        "Exit".into()
    };
    if let Some(font) = &art.large {
        let (glyphs, width) = font.layout(&name);
        let left = (160.0 - width / 2.0).round();
        for (x, image) in glyphs {
            w.ui.child(
                line,
                UiNode::image(image).at(left + x, 178.0, font.size.x, font.size.y),
            );
        }
    }
}

pub(crate) fn pick(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::Escape) {
        w.next_state = Some(AppState::Title);
        return;
    }
    for &(id, item) in &w.practice.items {
        if !w.ui.just_pressed(id) {
            continue;
        }
        if item == 20 {
            w.next_state = Some(AppState::Title);
        } else {
            w.current.number = level_of(item);
            w.current.loaded = None;
            w.next_state = Some(AppState::Briefing);
        }
    }
}
