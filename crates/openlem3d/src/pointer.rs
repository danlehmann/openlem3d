//! The mouse pointer, drawn from `MOUSE.RNC` as the original does
//! (`docs/spec/ui-graphics.md`, "Mouse pointer"): a cross-hair over menus
//! and the panel, a bracket over a lemming, and over the 3D view an arrow
//! chosen by which third of the view the pointer is in. Scaled like the
//! panel; the system cursor is hidden while it shows. The code screen has
//! no pointer (verified), and touch input never shows one.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::CursorOptions;
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

#[derive(Resource)]
struct Cells(Vec<Handle<Image>>);

#[derive(Component)]
struct Pointer;

fn load(mut commands: Commands, mut data: ResMut<Data>, mut images: ResMut<Assets<Image>>) {
    let d = &mut data.0;
    let (Ok(pal), Ok(raw)) = (d.palette("GFX/LM3D.PAL"), d.read(sheets::MOUSE.path)) else { return };
    let Ok(cells) = sheets::MOUSE.cut(&raw) else { return };
    let handles = cells
        .iter()
        .map(|c| {
            let mut image = Image::new(
                Extent3d { width: c.width as u32, height: c.height as u32, depth_or_array_layers: 1 },
                TextureDimension::D2,
                c.to_rgba(&pal, true),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            image.sampler = ImageSampler::nearest();
            images.add(image)
        })
        .collect();
    commands.insert_resource(Cells(handles));
    // Spawned last so it draws over every other UI node.
    commands.spawn((
        Pointer,
        ImageNode::default(),
        Node { position_type: PositionType::Absolute, ..default() },
        GlobalZIndex(i32::MAX),
        Pickable::IGNORE,
        Visibility::Hidden,
    ));
}

#[allow(clippy::too_many_arguments)]
fn update(
    cells: Option<Res<Cells>>,
    state: Res<State<AppState>>,
    game: Res<Game>,
    camera: Res<SceneCamera>,
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    mut windows: Query<(&Window, &mut CursorOptions)>,
    mut pointer: Query<(&mut ImageNode, &mut Node, &mut Visibility), With<Pointer>>,
) {
    let (Some(cells), Ok((window, mut cursor)), Ok((mut image, mut node, mut vis))) = (cells, windows.single_mut(), pointer.single_mut()) else {
        return;
    };
    let position = window.cursor_position().filter(|_| touches.iter().next().is_none());
    let cell = match (state.get(), position) {
        (AppState::Code, _) | (_, None) => None,
        (AppState::Playing, Some(p)) => Some(if ui.iter().any(|i| *i != Interaction::None) {
            CROSS_HAIR
        } else if game.sim.as_ref().and_then(|sim| crate::hud::lemming_at(sim, &camera, Vec2::new(window.width(), window.height()), p)).is_some() {
            BRACKET
        } else {
            let col = ((p.x / window.width() * 3.0) as usize).min(2);
            let row = ((p.y / window.height() * 3.0) as usize).min(2);
            GRID[row][col]
        }),
        (_, Some(_)) => Some(CROSS_HAIR),
    };
    let show = cell.is_some();
    if cursor.visible == show {
        cursor.visible = !show;
    }
    let (Some(cell), Some(p)) = (cell, position) else {
        *vis = Visibility::Hidden;
        return;
    };
    let size = CELL * (window.height() / 200.0).max(1.0);
    node.left = px(p.x - size / 2.0);
    node.top = px(p.y - size / 2.0);
    node.width = px(size);
    node.height = px(size);
    if let Some(h) = cells.0.get(cell)
        && image.image != *h
    {
        image.image = h.clone();
    }
    *vis = Visibility::Inherited;
}
