//! The UI: pictures and coloured rectangles, placed by hand on top of the
//! scene, and the buttons among them, as a tree of nodes in [`Ui`]. Every
//! node is positioned absolutely inside its parent, as the part of bevy_ui
//! the game first used, with the same behaviour:
//!
//! - layout in physical pixels, each edge rounded to a whole pixel, so that
//!   scaled pixel art lands on the same pixels as before;
//! - a node's children drawn after it, in order; the roots drawn by their
//!   [`UiNode::z`] and then in the order they were added;
//! - [`Interaction`] for every interactive node under the pointer (the
//!   cursor, else the first touch), from the top down to the first button,
//!   which takes it; other nodes let it through. Hidden nodes take none.
//!
//! A canvas ([`UiNode::canvas`]) is a rectangle of the original's pixels,
//! scaled to fit the screen and centred; nodes placed with [`UiNode::at`]
//! are given in its pixels.
//!
//! Drawing is one instanced wgpu pipeline, over the scene (`crate::gpu`).

use std::collections::HashMap;

use l3d_engine::input::Input;
use l3d_engine::prelude::*;

use crate::gpu::{Gpu, TARGET_FORMAT};
use crate::screen::Screen;

/// A length: none, in logical pixels, or a percentage of the parent's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Val {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}

impl Val {
    /// In physical pixels, against the parent's length; `Auto` is none.
    fn resolve(self, parent: f32, scale: f32) -> f32 {
        match self {
            Val::Auto => 0.0,
            Val::Px(v) => v * scale,
            Val::Percent(p) => parent * p / 100.0,
        }
    }
}

/// A number for [`px`] and [`percent`].
pub trait ValNum {
    fn to_f32(self) -> f32;
}

impl ValNum for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl ValNum for i32 {
    fn to_f32(self) -> f32 {
        self as f32
    }
}

pub fn px(v: impl ValNum) -> Val {
    Val::Px(v.to_f32())
}

pub fn percent(v: impl ValNum) -> Val {
    Val::Percent(v.to_f32())
}

/// Where a node is placed inside its parent (inside the parent's border); a
/// root is placed on the [`Screen`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    pub left: Val,
    pub top: Val,
    pub width: Val,
    pub height: Val,
    /// The border's width, on all four sides.
    pub border: Val,
    /// Whether the node clips its children to its inside.
    pub clip: bool,
}

/// A picture as large as fits in the node, keeping its shape, in the middle;
/// tinted by `color`.
#[derive(Clone, Debug)]
pub struct ImageNode {
    pub image: Handle<Image>,
    pub color: Color,
    pub flip_x: bool,
}

impl ImageNode {
    pub fn new(image: Handle<Image>) -> Self {
        ImageNode {
            image,
            color: Color::WHITE,
            flip_x: false,
        }
    }
}

/// What the pointer is doing to a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interaction {
    Pressed,
    Hovered,
    #[default]
    None,
}

/// Whether a node is drawn (and takes the pointer): as its parent is, or
/// not at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Inherited,
    Hidden,
}

/// Where a node ended up, after [`Ui::layout`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ComputedUi {
    /// In logical pixels, on the [`Screen`].
    pub rect: Rect,
    /// In physical pixels from the screen's corner, each edge rounded.
    pixels: Rect,
    /// The border's width in physical pixels.
    border: f32,
    /// Where the node's ancestors clip it, in physical pixels.
    clip: Option<Rect>,
    /// After its [`Visibility`] and its ancestors'.
    visible: bool,
}

/// A node of the UI. Build one with the methods, then add it to a [`Ui`].
#[derive(Clone, Debug, Default)]
pub struct UiNode {
    pub node: Node,
    /// Fills the node.
    pub background: Option<Color>,
    /// Colours the node's [`Node::border`].
    pub border_color: Option<Color>,
    pub image: Option<ImageNode>,
    pub visibility: Visibility,
    /// For roots: drawn above the roots with a lower one.
    pub z: i32,
    /// Takes the pointer from the nodes below it.
    pub button: bool,
    /// Has an [`Interaction`] (buttons always do).
    pub interactive: bool,
    pub interaction: Interaction,
    /// The interaction the frame before ([`Ui::focus`] keeps it).
    previous: Interaction,
    /// The node's rectangle in canvas pixels, relative to its parent.
    pub at: Option<Rect>,
    /// This node is the canvas, of this size in the original's pixels.
    pub canvas: Option<Vec2>,
    pub computed: ComputedUi,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// When it was added, ordering the roots.
    order: u64,
}

impl UiNode {
    pub fn new() -> Self {
        Self::default()
    }

    /// A node showing `image`.
    pub fn image(image: Handle<Image>) -> Self {
        UiNode {
            image: Some(ImageNode::new(image)),
            ..default()
        }
    }

    /// Places the node at `x`, `y`, `w`×`h` canvas pixels in its parent.
    pub fn at(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.at = Some(Rect::new(x, y, x + w, y + h));
        self
    }

    pub fn layout(mut self, node: Node) -> Self {
        self.node = node;
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, width: Val, color: Color) -> Self {
        self.node.border = width;
        self.border_color = Some(color);
        self
    }

    /// Tints the picture.
    pub fn tint(mut self, color: Color) -> Self {
        if let Some(i) = &mut self.image {
            i.color = color;
        }
        self
    }

    pub fn flip_x(mut self, flip: bool) -> Self {
        if let Some(i) = &mut self.image {
            i.flip_x = flip;
        }
        self
    }

    pub fn button(mut self) -> Self {
        self.button = true;
        self.interactive = true;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visibility = Visibility::Hidden;
        self
    }

    pub fn visible_if(mut self, shown: bool) -> Self {
        self.visibility = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        self
    }

    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }

    pub fn canvas(mut self, size: Vec2) -> Self {
        self.canvas = Some(size);
        self
    }

    pub fn set_visible(&mut self, shown: bool) {
        self.visibility = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    /// Shows `image` (if it isn't already).
    pub fn set_image(&mut self, image: &Handle<Image>) {
        match &mut self.image {
            Some(i) if i.image == *image => {}
            Some(i) => i.image = image.clone(),
            None => self.image = Some(ImageNode::new(image.clone())),
        }
    }

    pub fn set_tint(&mut self, color: Color) {
        if let Some(i) = &mut self.image {
            i.color = color;
        }
    }
}

/// A node's id in its [`Ui`]; it finds nothing once the node is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

#[derive(Default)]
struct Slot {
    generation: u32,
    node: Option<UiNode>,
}

/// Every node of the UI.
#[derive(Default)]
pub struct Ui {
    slots: Vec<Slot>,
    free: Vec<u32>,
    next_order: u64,
    /// Every node, back to front, as last laid out.
    stack: Vec<NodeId>,
    /// Nodes pressed and released in one frame, reset the next.
    reset: Vec<NodeId>,
}

impl Ui {
    /// Adds `node` as the last child of `parent`, or as a root.
    pub fn add(&mut self, parent: Option<NodeId>, mut node: UiNode) -> NodeId {
        node.parent = parent.filter(|&p| self.get(p).is_some());
        node.children.clear();
        node.order = self.next_order;
        self.next_order += 1;
        let index = match self.free.pop() {
            Some(i) => i,
            None => {
                self.slots.push(Slot::default());
                self.slots.len() as u32 - 1
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.node = Some(node);
        let id = NodeId {
            index,
            generation: slot.generation,
        };
        if let Some(p) = parent.and_then(|p| self.get_mut(p)) {
            p.children.push(id);
        }
        id
    }

    pub fn root(&mut self, node: UiNode) -> NodeId {
        self.add(None, node)
    }

    pub fn child(&mut self, parent: NodeId, node: UiNode) -> NodeId {
        self.add(Some(parent), node)
    }

    pub fn get(&self, id: NodeId) -> Option<&UiNode> {
        let slot = self.slots.get(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_ref())
            .flatten()
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut UiNode> {
        let slot = self.slots.get_mut(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_mut())
            .flatten()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(id).map_or(&[], |n| &n.children)
    }

    /// Removes the node and everything under it.
    pub fn remove(&mut self, id: NodeId) {
        let Some(node) = self.get(id) else { return };
        if let Some(p) = node.parent.and_then(|p| self.get_mut(p)) {
            p.children.retain(|&c| c != id);
        }
        let mut todo = vec![id];
        while let Some(id) = todo.pop() {
            let slot = &mut self.slots[id.index as usize];
            if let Some(n) = slot.node.take() {
                todo.extend(n.children);
                slot.generation = slot.generation.wrapping_add(1);
                self.free.push(id.index);
            }
        }
    }

    /// Removes the node `id` holds, if any, and forgets it.
    pub fn remove_opt(&mut self, id: &mut Option<NodeId>) {
        if let Some(id) = id.take() {
            self.remove(id);
        }
    }

    /// The roots, in the order added.
    pub fn roots(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(id, _)| id)
    }

    /// Every node, in no particular order.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &UiNode)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            let n = s.node.as_ref()?;
            Some((
                NodeId {
                    index: i as u32,
                    generation: s.generation,
                },
                n,
            ))
        })
    }

    pub fn interaction(&self, id: NodeId) -> Interaction {
        self.get(id).map_or(Interaction::None, |n| n.interaction)
    }

    pub fn pressed(&self, id: NodeId) -> bool {
        self.interaction(id) == Interaction::Pressed
    }

    /// Whether the node became pressed this frame.
    pub fn just_pressed(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| {
            n.interaction == Interaction::Pressed && n.previous != Interaction::Pressed
        })
    }

    /// Whether the node's interaction changed this frame.
    pub fn changed(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.interaction != n.previous)
    }

    /// Whether any node is pressed.
    pub fn any_pressed(&self) -> bool {
        self.nodes()
            .any(|(_, n)| n.interaction == Interaction::Pressed)
    }

    /// Whether the pointer is over (or pressing) any interactive node.
    pub fn any_active(&self) -> bool {
        self.nodes()
            .any(|(_, n)| n.interaction != Interaction::None)
    }

    pub fn set_visible(&mut self, id: NodeId, shown: bool) {
        if let Some(n) = self.get_mut(id) {
            n.set_visible(shown);
        }
    }

    pub fn set_image(&mut self, id: NodeId, image: &Handle<Image>) {
        if let Some(n) = self.get_mut(id) {
            n.set_image(image);
        }
    }

    /// The scale of the canvas, if there is one: screen pixels per canvas
    /// pixel.
    fn canvas_scale(&self, screen: Vec2) -> Option<f32> {
        let size = self.nodes().find_map(|(_, n)| n.canvas)?;
        Some((screen.x / size.x).min(screen.y / size.y))
    }

    /// Places the canvas and the nodes given in its pixels, then every node,
    /// root by root, and stacks them.
    pub fn layout(&mut self, screen: &Screen, scale: f32) {
        let (w, h) = (screen.size.x, screen.size.y);
        if let Some(s) = self.canvas_scale(screen.size) {
            for slot in &mut self.slots {
                let Some(n) = &mut slot.node else { continue };
                if let Some(size) = n.canvas {
                    n.node.left = px((w - size.x * s) / 2.0);
                    n.node.top = px((h - size.y * s) / 2.0);
                    n.node.width = px(size.x * s);
                    n.node.height = px(size.y * s);
                } else if let Some(a) = n.at {
                    n.node.left = px(a.min.x * s);
                    n.node.top = px(a.min.y * s);
                    n.node.width = px(a.width() * s);
                    n.node.height = px(a.height() * s);
                }
            }
        }
        let mut order: Vec<_> = self
            .nodes()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(id, n)| (n.z, n.order, id))
            .collect();
        order.sort_by_key(|&(z, o, _)| (z, o));
        self.stack.clear();
        // Depth first. Rounded as taffy (bevy_ui's layout) rounds: a node's
        // offset from its parent's corner on its own, its size from where its
        // edges fall unrounded.
        let mut todo: Vec<_> = order
            .iter()
            .rev()
            .map(|&(_, _, id)| (id, Parent::screen(screen.size * scale)))
            .collect();
        while let Some((id, parent)) = todo.pop() {
            let Some(n) = self.get_mut(id) else { continue };
            let node = n.node;
            let offset = parent.border
                + Vec2::new(
                    node.left.resolve(parent.inside.x, scale),
                    node.top.resolve(parent.inside.y, scale),
                );
            let size = Vec2::new(
                node.width.resolve(parent.inside.x, scale),
                node.height.resolve(parent.inside.y, scale),
            );
            let exact = parent.exact + offset;
            let min = parent.min + round(offset);
            let pixels = Rect::from_corners(min, min + round(exact + size) - round(exact));
            let border = node.border.resolve(parent.inside.x, scale);
            let visible = parent.visible && n.visibility == Visibility::Inherited;
            n.computed = ComputedUi {
                rect: Rect::from_corners(exact / scale, (exact + size) / scale),
                pixels,
                border,
                clip: parent.clip,
                visible,
            };
            let clip = if node.clip {
                let own = Rect::from_corners(pixels.min + border, pixels.max - border);
                Some(parent.clip.map_or(own, |c| c.intersect(own)))
            } else {
                parent.clip
            };
            let inside = Parent {
                min,
                exact,
                border: Vec2::splat(border),
                inside: size - 2.0 * border,
                clip,
                visible,
            };
            for &child in n.children.iter().rev() {
                todo.push((child, inside));
            }
            self.stack.push(id);
        }
    }

    /// Sets [`Interaction`] as bevy_ui's focus system did: press on a click
    /// or touch, release when let go or when the pointer leaves.
    pub fn focus(&mut self, input: &Input, window: &Window, screen: &Screen) {
        for slot in &mut self.slots {
            if let Some(n) = &mut slot.node {
                n.previous = n.interaction;
            }
        }
        // A press and release in the same frame shows as pressed for one
        // frame.
        for id in std::mem::take(&mut self.reset) {
            if let Some(n) = self.get_mut(id) {
                n.interaction = Interaction::None;
            }
        }
        let (mouse, touches) = (&input.mouse, &input.touches);
        let released = mouse.just_released(MouseButton::Left) || touches.any_just_released();
        if released {
            for slot in &mut self.slots {
                if let Some(n) = &mut slot.node
                    && n.interaction == Interaction::Pressed
                {
                    n.interaction = Interaction::None;
                }
            }
        }
        let clicked = mouse.just_pressed(MouseButton::Left) || touches.any_just_pressed();
        let pointer = window
            .cursor_position()
            .or_else(|| touches.first_pressed_position())
            .map(|p| screen.on_screen(p) * window.scale_factor());
        let mut taken = false;
        let mut reset = Vec::new();
        for i in (0..self.stack.len()).rev() {
            let id = self.stack[i];
            let Some(n) = self.get_mut(id) else { continue };
            if !n.interactive {
                continue;
            }
            let c = n.computed;
            if !c.visible {
                n.interaction = Interaction::None;
                continue;
            }
            let over = pointer
                .filter(|&p| c.pixels.contains(p) && c.clip.is_none_or(|clip| clip.contains(p)));
            if over.is_none() {
                if n.interaction == Interaction::Hovered || pointer.is_none() {
                    n.interaction = Interaction::None;
                }
                continue;
            }
            // Below the button that took the pointer.
            if taken {
                if n.interaction != Interaction::Pressed {
                    n.interaction = Interaction::None;
                }
                continue;
            }
            if clicked {
                if n.interaction != Interaction::Pressed {
                    n.interaction = Interaction::Pressed;
                    if released {
                        reset.push(id);
                    }
                }
            } else if n.interaction == Interaction::None {
                n.interaction = Interaction::Hovered;
            }
            taken = n.button;
        }
        self.reset = reset;
    }

    /// The rectangles to draw, back to front: per node its background, its
    /// picture and its border.
    fn quads(&self) -> Vec<Quad> {
        let mut out = Vec::new();
        for &id in &self.stack {
            let Some(n) = self.get(id) else { continue };
            let c = n.computed;
            if !c.visible {
                continue;
            }
            let mut quad = |rect: Rect, image: Option<&ImageNode>, color: Color| {
                if rect.is_empty() || color.alpha() <= 0.0 {
                    return;
                }
                out.push(Quad {
                    rect,
                    clip: c.clip,
                    image: image.map(|i| i.image.id()),
                    flip_x: image.is_some_and(|i| i.flip_x),
                    color: color.to_linear(),
                });
            };
            let r = c.pixels;
            if let Some(bg) = n.background {
                quad(r, None, bg);
            }
            if let Some(i) = &n.image {
                quad(r, Some(i), i.color);
            }
            if let Some(b) = n.border_color
                && c.border > 0.0
            {
                let w = c.border.round();
                let (min, max) = (r.min, r.max);
                for side in [
                    Rect::new(min.x, min.y, max.x, min.y + w),
                    Rect::new(min.x, max.y - w, max.x, max.y),
                    Rect::new(min.x, min.y + w, min.x + w, max.y - w),
                    Rect::new(max.x - w, min.y + w, max.x, max.y - w),
                ] {
                    quad(side, None, b);
                }
            }
        }
        out
    }
}

/// What a node is placed in, in physical pixels.
#[derive(Clone, Copy)]
struct Parent {
    /// The corner, rounded as laid out.
    min: Vec2,
    /// The corner before rounding.
    exact: Vec2,
    /// From the corner to the inside.
    border: Vec2,
    /// The size of the inside.
    inside: Vec2,
    clip: Option<Rect>,
    visible: bool,
}

impl Parent {
    fn screen(size: Vec2) -> Self {
        Parent {
            min: Vec2::ZERO,
            exact: Vec2::ZERO,
            border: Vec2::ZERO,
            inside: size,
            clip: None,
            visible: true,
        }
    }
}

/// To the nearest whole pixel, halves up (taffy's rounding).
fn round(v: Vec2) -> Vec2 {
    (v + 0.5).floor()
}

/// One rectangle to draw: where (physical pixels from the screen's corner),
/// where it is clipped, its picture (white without one) and colour.
struct Quad {
    rect: Rect,
    clip: Option<Rect>,
    image: Option<AssetId<Image>>,
    flip_x: bool,
    color: LinearRgba,
}

/// Floats per instance: rectangle, texture coordinates, colour, rectangle in
/// pixels.
const QUAD_FLOATS: usize = 16;

pub struct UiGpu {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    /// Every picture is pixel art, sampled sharp.
    sampler: wgpu::Sampler,
    /// A white texel, for plain colours.
    white: wgpu::BindGroup,
    /// Per-frame instances (grown as needed).
    quads: Option<wgpu::Buffer>,
    /// The pictures drawn so far.
    images: HashMap<AssetId<Image>, Uploaded>,
}

/// A picture on the GPU.
struct Uploaded {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl UiGpu {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: (QUAD_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui"),
            // Linear, sampled by `ui.wgsl`'s `sharp_uv` so that texels stay
            // square at any scale.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = texture(gpu, UVec2::ONE, wgpu::TextureFormat::Rgba8Unorm);
        write(gpu, &white, &[255; 4]);
        let white = bind_group(
            device,
            &layout,
            &white.create_view(&Default::default()),
            &sampler,
        );
        UiGpu {
            layout,
            pipeline,
            sampler,
            white,
            quads: None,
            images: HashMap::new(),
        }
    }

    /// The picture `id` on the GPU, uploaded now if it isn't yet.
    fn image(
        &mut self,
        gpu: &Gpu,
        id: AssetId<Image>,
        images: &Assets<Image>,
    ) -> Option<&Uploaded> {
        if !self.images.contains_key(&id) {
            let image = images.get(id)?;
            let texture = texture(gpu, image.size(), wgpu::TextureFormat::Rgba8UnormSrgb);
            write(gpu, &texture, &image.data);
            let bind_group = bind_group(
                &gpu.device,
                &self.layout,
                &texture.create_view(&Default::default()),
                &self.sampler,
            );
            self.images.insert(
                id,
                Uploaded {
                    texture,
                    bind_group,
                },
            );
        }
        self.images.get(&id)
    }

    /// Forgets pictures that changed or went away; changed ones of the same
    /// size and format are written over instead.
    fn update_images(&mut self, gpu: &Gpu, images: &mut Assets<Image>) {
        for change in images.take_events() {
            let (AssetEvent::Modified { id } | AssetEvent::Removed { id }) = change else {
                continue;
            };
            let Some(old) = self.images.get(&id) else {
                continue;
            };
            match images.get(id) {
                Some(image) if image.size() == texture_size(old) => {
                    write(gpu, &old.texture, &image.data);
                }
                _ => {
                    self.images.remove(&id);
                }
            }
        }
    }

    /// Draws the UI into `viewport` (x, y, width, height) of `target`, in
    /// order, a call per run of rectangles sharing a picture and a clip.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: UVec4,
        ui: &Ui,
        images: &mut Assets<Image>,
    ) {
        self.update_images(gpu, images);
        let (origin, size) = (
            Vec2::new(viewport.x as f32, viewport.y as f32),
            Vec2::new(viewport.z.max(1) as f32, viewport.w.max(1) as f32),
        );
        // The rectangles that can be drawn (their pictures exist), with their
        // scissor rectangles in the target.
        let quads = ui.quads();
        let mut runs: Vec<(Option<AssetId<Image>>, URect, std::ops::Range<u32>)> = Vec::new();
        let mut data: Vec<f32> = Vec::with_capacity(quads.len() * QUAD_FLOATS);
        for q in &quads {
            let mut rect = q.rect;
            if let Some(id) = q.image {
                let Some(image) = self.image(gpu, id, images) else {
                    continue;
                };
                // As large as fits, keeping its shape, in the middle.
                let source = texture_size(image).as_vec2();
                rect = Rect::from_center_size(
                    rect.center(),
                    source * (rect.size() / source).min_element(),
                );
            }
            let clip = q
                .clip
                .unwrap_or(Rect::from_corners(Vec2::ZERO, size))
                .intersect(Rect::from_corners(Vec2::ZERO, size))
                .intersect(rect);
            if clip.is_empty() {
                continue;
            }
            let scissor = URect::from_corners(
                (clip.min + origin).floor().as_uvec2(),
                (clip.max + origin).ceil().as_uvec2(),
            );
            // As the orthographic projection bevy_ui used computes it, so that
            // pixels exactly between two texels pick the same one.
            let ndc = |p: Vec2| Vec2::new(p.x * (2.0 / size.x) - 1.0, p.y * (-2.0 / size.y) + 1.0);
            let (a, b) = (ndc(rect.min), ndc(rect.max));
            let (u0, u1) = if q.flip_x { (1.0, 0.0) } else { (0.0, 1.0) };
            let c = q.color.to_f32_array();
            let (min, max) = (rect.min + origin, rect.max + origin);
            data.extend_from_slice(&[
                a.x, a.y, b.x, b.y, u0, 0.0, u1, 1.0, c[0], c[1], c[2], c[3], min.x, min.y, max.x,
                max.y,
            ]);
            let i = (data.len() / QUAD_FLOATS) as u32 - 1;
            match runs.last_mut() {
                Some((image, s, range)) if *image == q.image && *s == scissor => range.end = i + 1,
                _ => runs.push((q.image, scissor, i..i + 1)),
            }
        }
        if runs.is_empty() {
            return;
        }
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        if self
            .quads
            .as_ref()
            .is_none_or(|b| b.size() < bytes.len() as u64)
        {
            self.quads = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui quads"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let quads = self.quads.as_ref().expect("made above");
        gpu.queue.write_buffer(quads, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_viewport(origin.x, origin.y, size.x, size.y, 0.0, 1.0);
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, quads.slice(..));
        for (image, scissor, range) in runs {
            let group = match image {
                Some(id) => &self.images[&id].bind_group,
                None => &self.white,
            };
            pass.set_bind_group(0, group, &[]);
            let s = scissor.size();
            pass.set_scissor_rect(scissor.min.x, scissor.min.y, s.x, s.y);
            pass.draw(0..4, range);
        }
        // The Pixel 11 Pro's PowerVR driver keeps only what lies inside the
        // scissor rectangle set when the pass ends, losing the rest of the pass.
        pass.set_scissor_rect(
            origin.x as u32,
            origin.y as u32,
            size.x as u32,
            size.y as u32,
        );
    }
}

fn texture_size(image: &Uploaded) -> UVec2 {
    let s = image.texture.size();
    UVec2::new(s.width, s.height)
}

/// A texture of `size` in `format` to upload pictures into.
fn texture(gpu: &Gpu, size: UVec2, format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui image"),
        size: wgpu::Extent3d {
            width: size.x.max(1),
            height: size.y.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Writes `data`, rows of texels, into the whole of `texture`.
fn write(gpu: &Gpu, texture: &wgpu::Texture, data: &[u8]) {
    let texel = texture.format().block_copy_size(None).unwrap_or(4);
    gpu.queue.write_texture(
        texture.as_image_copy(),
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture.width() * texel),
            rows_per_image: None,
        },
        texture.size(),
    );
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ui image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
