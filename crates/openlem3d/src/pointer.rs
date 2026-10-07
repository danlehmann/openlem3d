//! The mouse pointer, drawn from `MOUSE.RNC` as the original does
//! (`docs/spec/ui-graphics.md`, "Mouse pointer"): a cross-hair over menus
//! and the panel, a bracket over a lemming, and over the 3D view an arrow
//! chosen by which third of the view the pointer is in. Scaled like the
//! panel. It is the system's hardware cursor with our image, so it moves
//! at the system's rate whatever the game's frame rate. The code screen
//! has no pointer (verified; the system's default shows there), and touch
//! input never shows one.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage, SystemCursorIcon};
use l3d_formats::sheets;

use crate::menu::AppState;
use crate::scene_render::SceneCamera;
use crate::{Data, Game};

pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, update);
    }
}

/// `MOUSE.RNC` cells.
const CROSS_HAIR: usize = 9;
const BRACKET: usize = 8;
/// The 3×3 grid over the 3D view, row by row from the top (verified).
const GRID: [[usize; 3]; 3] = [[7, 1, 6], [0, 9, 2], [3, 4, 5]];
const CELL: f32 = 16.0;

/// The pointer cells as RGBA, with each cell's size, and the cursor images
/// made from them so far, by cell and drawn size.
#[derive(Resource)]
struct Cells {
    rgba: Vec<(Vec<u8>, UVec2)>,
    scaled: HashMap<(usize, u32), Handle<Image>>,
}

fn load(mut commands: Commands, mut data: ResMut<Data>) {
    let d = &mut data.0;
    let (Ok(pal), Ok(raw)) = (d.palette("GFX/LM3D.PAL"), d.read(sheets::MOUSE.path)) else { return };
    let Ok(cells) = sheets::MOUSE.cut(&raw) else { return };
    let rgba = cells.iter().map(|c| (c.to_rgba(&pal, true), UVec2::new(c.width as u32, c.height as u32))).collect();
    commands.insert_resource(Cells { rgba, scaled: HashMap::new() });
}

/// Cell `cell` scaled (nearest neighbour) to `size`×`size` pixels.
fn scaled(cells: &mut Cells, cell: usize, size: u32, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    if let Some(h) = cells.scaled.get(&(cell, size)) {
        return Some(h.clone());
    }
    let (src, dim) = cells.rgba.get(cell)?;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (sx, sy) = (x * dim.x / size, y * dim.y / size);
            let s = ((sy * dim.x + sx) * 4) as usize;
            let d = ((y * size + x) * 4) as usize;
            px[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    let image = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    let h = images.add(image);
    cells.scaled.insert((cell, size), h.clone());
    Some(h)
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    cells: Option<ResMut<Cells>>,
    state: Res<State<AppState>>,
    game: Res<Game>,
    camera: Res<SceneCamera>,
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    windows: Query<(Entity, &Window, Option<&CursorIcon>)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(mut cells), Ok((entity, window, icon))) = (cells, windows.single()) else { return };
    let position = window.cursor_position().filter(|_| touches.iter().next().is_none());
    let size = Vec2::new(window.width(), window.height());
    let cell = match (state.get(), position) {
        (AppState::Code, _) | (_, None) => None,
        (AppState::Playing, Some(p)) => Some(if ui.iter().any(|i| *i != Interaction::None) {
            CROSS_HAIR
        } else if game.sim.as_ref().and_then(|sim| crate::hud::lemming_at(sim, &camera, size, p)).is_some() {
            BRACKET
        } else {
            let [col, row] = crate::pointer_region(p, size);
            GRID[row][col]
        }),
        (_, Some(_)) => Some(CROSS_HAIR),
    };
    // Physical pixels: the cursor image is not scaled by the system.
    let pixels = (CELL * (window.physical_height() as f32 / 200.0).max(1.0)).round() as u32;
    let wanted = match cell.and_then(|c| scaled(&mut cells, c, pixels, &mut images)) {
        Some(handle) => CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            handle,
            hotspot: ((pixels / 2) as u16, (pixels / 2) as u16),
            ..default()
        })),
        None => CursorIcon::System(SystemCursorIcon::Default),
    };
    if icon != Some(&wanted) {
        commands.entity(entity).insert(wanted);
    }
}
