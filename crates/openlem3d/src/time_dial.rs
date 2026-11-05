//! The three-finger time dial's overlay (ours, `touch::TimeDial`): while it
//! is held the picture dims towards its edges, and a bar across the middle
//! shows the speed, paused at its centre, running forward to the right and
//! back to the left, with a red fill from the centre to the speed chosen.

use l3d_engine::prelude::*;

use crate::World;
use crate::texture::rgba_image;
use crate::touch::{DIAL_MAX, DIAL_REST, DIAL_UNIT};
use crate::ui::{Node, NodeId, UiNode, percent, px};

/// The speeds marked on the bar, each side of the centre.
const MARKS: [(f32, &str); 4] = [(0.5, ""), (1.0, "1x"), (2.0, "2x"), (3.0, "3x")];
/// The bar's height and its place down the screen, as fractions of the
/// screen height.
const BAR_HEIGHT: f32 = 0.025;
const BAR_Y: f32 = 0.5;
/// Height of the vignette image; it is made at the screen's shape, as
/// pictures keep theirs.
const VIGNETTE_SIZE: u32 = 128;

#[derive(Default)]
pub struct Overlay {
    root: Option<NodeId>,
    vignette: Option<NodeId>,
    /// The width the vignette was made at, for [`VIGNETTE_SIZE`] high.
    vignette_width: u32,
    track: Option<NodeId>,
    fill: Option<NodeId>,
    centre: Option<NodeId>,
    marks: Vec<Mark>,
    /// The readout above the bar, and what it shows.
    readout: Option<(NodeId, String)>,
}

/// A speed marked on the bar: its tick, and its label's glyphs with their
/// x offsets from the tick.
struct Mark {
    speed: f32,
    tick: NodeId,
    glyphs: Vec<(NodeId, f32)>,
}

/// How far from the centre (pixels) speed `v` lies, as the fingers move.
fn offset(v: f32, height: f32) -> f32 {
    if v == 0.0 {
        0.0
    } else {
        (DIAL_REST + v.abs() * DIAL_UNIT) * height * v.signum()
    }
}

/// Black, clear in the middle and darker towards the edges, `width` wide.
fn vignette(width: u32) -> Image {
    let n = VIGNETTE_SIZE;
    let mut px = Vec::with_capacity((width * n * 4) as usize);
    for y in 0..n {
        for x in 0..width {
            let (u, v) = (
                (x as f32 + 0.5) / width as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / n as f32 * 2.0 - 1.0,
            );
            let r = (u * u + v * v).sqrt() / std::f32::consts::SQRT_2;
            let t = ((r - 0.2) / 0.8).clamp(0.0, 1.0);
            let a = 0.3 + 0.5 * t * t * (3.0 - 2.0 * t);
            px.extend_from_slice(&[0, 0, 0, (a * 255.0) as u8]);
        }
    }
    rgba_image(width, n, px)
}

fn place(ui: &mut crate::ui::Ui, id: Option<NodeId>, x: f32, y: f32, w: f32, h: f32) {
    if let Some(n) = id.and_then(|id| ui.get_mut(id)) {
        n.node.left = px(x);
        n.node.top = px(y);
        n.node.width = px(w);
        n.node.height = px(h);
    }
}

/// Builds the overlay the first time the dial is held.
fn spawn(w: &mut World) {
    let ui = &mut w.ui;
    let root = ui.root(
        UiNode::new()
            .layout(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .z(100)
            .hidden(),
    );
    let o = &mut w.time_overlay;
    o.vignette = Some(ui.child(root, UiNode::new()));
    o.root = Some(root);
    o.track = Some(
        ui.child(
            root,
            UiNode::new()
                .background(Color::srgba(0.0, 0.0, 0.0, 0.6))
                .border(px(1), Color::srgba(1.0, 1.0, 1.0, 0.7)),
        ),
    );
    o.fill = Some(ui.child(
        root,
        UiNode::new().background(Color::srgb(0.85, 0.08, 0.05)),
    ));
    o.centre = Some(ui.child(root, UiNode::new().background(Color::WHITE)));
    let font = w.art.as_ref().and_then(|a| a.large.as_ref());
    for side in [-1.0, 1.0] {
        for (v, label) in MARKS {
            let tick = ui.child(
                root,
                UiNode::new().background(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            );
            let glyphs = font.map_or(Vec::new(), |f| {
                let (glyphs, width) = f.layout(label);
                glyphs
                    .into_iter()
                    .map(|(x, image)| (ui.child(root, UiNode::image(image)), x - width / 2.0))
                    .collect()
            });
            o.marks.push(Mark {
                speed: v * side,
                tick,
                glyphs,
            });
        }
    }
    o.readout = Some((ui.child(root, UiNode::new()), String::new()));
}

/// Shows the overlay while the dial is held (or Backspace runs the level
/// back), laid out for the screen.
pub(crate) fn update(w: &mut World) {
    let speed = w.game.dial.or(crate::backspace_rewind(w));
    if speed.is_some() && w.time_overlay.root.is_none() {
        spawn(w);
    }
    let Some(root) = w.time_overlay.root else {
        return;
    };
    w.ui.set_visible(root, speed.is_some());
    let Some(v) = speed else {
        return;
    };
    let (sw, sh) = (w.screen.size.x, w.screen.size.y);
    let scale = (sh / 200.0).max(1.0);
    let (cx, cy) = (sw / 2.0, sh * BAR_Y);
    let half = offset(DIAL_MAX, sh);
    let bar = (BAR_HEIGHT * sh).max(6.0);
    let width = ((VIGNETTE_SIZE as f32 * sw / sh).round() as u32).max(1);
    if width != w.time_overlay.vignette_width
        && let Some(id) = w.time_overlay.vignette
    {
        w.time_overlay.vignette_width = width;
        let image = w.e.images.add(vignette(width));
        if let Some(n) = w.ui.get_mut(id) {
            *n = UiNode::image(image);
        }
    }
    let ui = &mut w.ui;
    let o = &mut w.time_overlay;
    place(ui, o.vignette, 0.0, 0.0, sw, sh);
    place(
        ui,
        o.track,
        cx - half - 2.0,
        cy - bar / 2.0 - 2.0,
        2.0 * half + 4.0,
        bar + 4.0,
    );
    let to = offset(v, sh);
    place(ui, o.fill, cx + to.min(0.0), cy - bar / 2.0, to.abs(), bar);
    place(ui, o.centre, cx - 1.5, cy - bar, 3.0, 2.0 * bar);
    let font_h = w
        .art
        .as_ref()
        .and_then(|a| a.large.as_ref())
        .map_or(10.0, |f| f.size.y);
    for mark in &o.marks {
        let x = cx + offset(mark.speed, sh);
        place(ui, Some(mark.tick), x - 1.0, cy + bar / 2.0, 2.0, bar * 0.6);
        let size = w
            .art
            .as_ref()
            .and_then(|a| a.large.as_ref())
            .map_or(Vec2::splat(10.0), |f| f.size);
        for &(g, gx) in &mark.glyphs {
            place(
                ui,
                Some(g),
                x + gx * scale * 0.5,
                cy + bar + 4.0,
                size.x * scale * 0.5,
                size.y * scale * 0.5,
            );
        }
    }
    // The readout: what the dial is doing.
    let text = if v == 0.0 {
        "Paused".to_string()
    } else {
        let n = format!("{:.1}", v.abs());
        let n = n.trim_end_matches(".0");
        if v < 0.0 {
            format!("Rewind {n}x")
        } else {
            format!("{n}x")
        }
    };
    let Some((node, shown)) = &mut o.readout else {
        return;
    };
    let Some(font) = w.art.as_ref().and_then(|a| a.large.as_ref()) else {
        return;
    };
    let node = *node;
    if *shown != text {
        *shown = text.clone();
        for c in ui.children(node).to_vec() {
            ui.remove(c);
        }
        for (_, image) in font.layout(&text).0 {
            ui.child(node, UiNode::image(image));
        }
    }
    let (layout, width) = font.layout(&text);
    let top = cy - bar - font_h * scale - 6.0;
    for (&child, (x, _)) in ui.children(node).to_vec().iter().zip(layout) {
        place(
            ui,
            Some(child),
            cx + (x - width / 2.0) * scale,
            top,
            font.size.x * scale,
            font.size.y * scale,
        );
    }
}
