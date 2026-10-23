//! The UI: pictures and coloured rectangles, placed by hand on top of the
//! scene, and the buttons among them. Every node is positioned absolutely
//! inside its parent, so this is a small stand-in for the part of bevy_ui
//! the game used, with the same names and the same behaviour:
//!
//! - layout in physical pixels, each edge rounded to a whole pixel, so that
//!   scaled pixel art lands on the same pixels as before;
//! - a node's children drawn after it, in order; nodes without a parent, and
//!   ones with a [`GlobalZIndex`], drawn by that index and then in the order
//!   they were spawned;
//! - [`Interaction`] for every node under the pointer (the cursor, else the
//!   first touch), from the top down to the first [`Button`], which takes it;
//!   other nodes let it through. Hidden nodes take none.
//!
//! Drawing is one instanced wgpu pipeline, over the scene (`crate::gpu`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::gpu::{Gpu, TARGET_FORMAT};
use crate::screen::Screen;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStack>()
            .add_systems(
                PreUpdate,
                focus.in_set(UiSystems::Focus).after(InputSystems),
            )
            .add_systems(PostUpdate, layout.in_set(UiSystems::Layout));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiSystems {
    /// Sets [`Interaction`] from the pointer (PreUpdate).
    Focus,
    /// Places every node (PostUpdate).
    Layout,
}

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

/// Whether a node clips its children to its inside.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overflow {
    clip: bool,
}

impl Overflow {
    pub fn clip() -> Self {
        Overflow { clip: true }
    }
}

/// A rectangle of the UI, placed inside its parent's (inside the parent's
/// border); a node without a parent is placed on the [`Screen`].
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[require(ComputedUi, SpawnOrder, Visibility)]
pub struct Node {
    pub left: Val,
    pub top: Val,
    pub width: Val,
    pub height: Val,
    /// The border's width, on all four sides.
    pub border: Val,
    pub overflow: Overflow,
}

/// Fills the node.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BackgroundColor(pub Color);

/// Colours the node's [`Node::border`].
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BorderColor(pub Color);

/// A picture as large as fits in the node, keeping its shape, in the middle;
/// tinted by `color`. None by default.
#[derive(Component, Clone, Debug)]
#[require(Node)]
pub struct ImageNode {
    pub image: Handle<Image>,
    pub color: Color,
    pub flip_x: bool,
}

impl Default for ImageNode {
    fn default() -> Self {
        ImageNode::new(bevy::image::TRANSPARENT_IMAGE_HANDLE)
    }
}

impl ImageNode {
    pub fn new(image: Handle<Image>) -> Self {
        ImageNode {
            image,
            color: Color::WHITE,
            flip_x: false,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

/// What the pointer is doing to a node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interaction {
    Pressed,
    Hovered,
    #[default]
    None,
}

/// A node that takes the pointer from the nodes below it.
#[derive(Component, Clone, Copy, Debug, Default)]
#[require(Node, Interaction)]
pub struct Button;

/// Whether a node is drawn (and takes the pointer): as its parent is, or
/// not at all.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Inherited,
    Hidden,
}

/// Draws the node and its children above the other roots: by this index,
/// then in the order they were spawned. Only for nodes without a parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalZIndex(pub i32);

/// Where a node ended up, after [`UiSystems::Layout`].
#[derive(Component, Clone, Copy, Debug, Default)]
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

/// When a node was spawned, ordering the roots.
#[derive(Component, Clone, Copy, Debug)]
struct SpawnOrder(u64);

impl Default for SpawnOrder {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        SpawnOrder(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Every node, back to front.
#[derive(Resource, Default)]
struct UiStack(Vec<Entity>);

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

/// Places the nodes, root by root, and stacks them.
#[allow(clippy::type_complexity)] // Bevy system parameters
fn layout(
    screen: Res<Screen>,
    windows: Query<&Window>,
    roots: Query<(Entity, &SpawnOrder, Option<&GlobalZIndex>), (With<Node>, Without<ChildOf>)>,
    nodes: Query<(&Node, &Visibility, Option<&Children>)>,
    mut computed: Query<&mut ComputedUi>,
    mut stack: ResMut<UiStack>,
) {
    let scale = windows.iter().next().map_or(1.0, |w| w.scale_factor());
    let mut order: Vec<_> = roots
        .iter()
        .map(|(e, s, z)| (z.copied().unwrap_or_default(), s.0, e))
        .collect();
    order.sort();
    stack.0.clear();
    // Depth first. Rounded as taffy (bevy_ui's layout) rounds: a node's
    // offset from its parent's corner on its own, its size from where its
    // edges fall unrounded.
    let mut todo: Vec<_> = order
        .iter()
        .rev()
        .map(|&(_, _, e)| (e, Parent::screen(screen.size * scale)))
        .collect();
    while let Some((e, parent)) = todo.pop() {
        let Ok((node, visibility, children)) = nodes.get(e) else {
            continue;
        };
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
        let visible = parent.visible && *visibility == Visibility::Inherited;
        if let Ok(mut c) = computed.get_mut(e) {
            *c = ComputedUi {
                rect: Rect::from_corners(exact / scale, (exact + size) / scale),
                pixels,
                border,
                clip: parent.clip,
                visible,
            };
        }
        stack.0.push(e);
        let clip = if node.overflow.clip {
            let own = Rect::from_corners(pixels.min + border, pixels.max - border);
            Some(parent.clip.map_or(own, |c| c.intersect(own)))
        } else {
            parent.clip
        };
        if let Some(children) = children {
            let inside = Parent {
                min,
                exact,
                border: Vec2::splat(border),
                inside: size - 2.0 * border,
                clip,
                visible,
            };
            for child in children.iter().rev() {
                todo.push((child, inside));
            }
        }
    }
}

/// To the nearest whole pixel, halves up (taffy's rounding).
fn round(v: Vec2) -> Vec2 {
    (v + 0.5).floor()
}

/// Sets [`Interaction`] as bevy_ui's focus system did: press on a click or
/// touch, release when let go or when the pointer leaves.
fn focus(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    screen: Res<Screen>,
    stack: Res<UiStack>,
    mut nodes: Query<(&ComputedUi, Option<&mut Interaction>, Has<Button>)>,
    mut reset: Local<Vec<Entity>>,
) {
    // A press and release in the same frame shows as pressed for one frame.
    for e in reset.drain(..) {
        if let Ok((_, Some(mut i), _)) = nodes.get_mut(e) {
            *i = Interaction::None;
        }
    }
    let released = mouse.just_released(MouseButton::Left) || touches.any_just_released();
    if released {
        for (_, i, _) in &mut nodes {
            if let Some(mut i) = i
                && *i == Interaction::Pressed
            {
                *i = Interaction::None;
            }
        }
    }
    let clicked = mouse.just_pressed(MouseButton::Left) || touches.any_just_pressed();
    let pointer = windows.iter().next().and_then(|w| {
        let p = w
            .cursor_position()
            .or_else(|| touches.first_pressed_position())?;
        Some(screen.on_screen(p) * w.scale_factor())
    });
    let mut taken = false;
    for &e in stack.0.iter().rev() {
        let Ok((c, interaction, button)) = nodes.get_mut(e) else {
            continue;
        };
        let Some(mut i) = interaction else {
            continue;
        };
        if !c.visible {
            i.set_if_neq(Interaction::None);
            continue;
        }
        let over =
            pointer.filter(|&p| c.pixels.contains(p) && c.clip.is_none_or(|clip| clip.contains(p)));
        if over.is_none() {
            if *i == Interaction::Hovered || pointer.is_none() {
                i.set_if_neq(Interaction::None);
            }
            continue;
        }
        // Below the button that took the pointer.
        if taken {
            if *i != Interaction::Pressed {
                i.set_if_neq(Interaction::None);
            }
            continue;
        }
        if clicked {
            if *i != Interaction::Pressed {
                *i = Interaction::Pressed;
                if released {
                    reset.push(e);
                }
            }
        } else if *i == Interaction::None {
            *i = Interaction::Hovered;
        }
        taken = button;
    }
}

/// What a node draws: where it is, and its colours and picture.
type Drawn = (
    &'static ComputedUi,
    Option<&'static BackgroundColor>,
    Option<&'static BorderColor>,
    Option<&'static ImageNode>,
);

/// What a frame draws of the UI.
#[derive(SystemParam)]
pub struct UiInput<'w, 's> {
    stack: Res<'w, UiStack>,
    nodes: Query<'w, 's, Drawn>,
    images: Res<'w, Assets<Image>>,
    changes: MessageReader<'w, 's, AssetEvent<Image>>,
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

impl UiInput<'_, '_> {
    /// The rectangles to draw, back to front: per node its background, its
    /// picture and its border.
    fn quads(&self) -> Vec<Quad> {
        let mut out = Vec::new();
        for &e in &self.stack.0 {
            let Ok((c, background, border, image)) = self.nodes.get(e) else {
                continue;
            };
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
            if let Some(bg) = background {
                quad(r, None, bg.0);
            }
            if let Some(i) = image {
                quad(r, Some(i), i.color);
            }
            if let Some(b) = border
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
                    quad(side, None, b.0);
                }
            }
        }
        out
    }
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
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
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
            let data = image.data.as_ref()?;
            let texture = texture(gpu, image.size(), image.texture_descriptor.format);
            write(gpu, &texture, data);
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
    fn update_images(&mut self, gpu: &Gpu, input: &mut UiInput) {
        for change in input.changes.read() {
            let (AssetEvent::Modified { id } | AssetEvent::Removed { id }) = *change else {
                continue;
            };
            let Some(old) = self.images.get(&id) else {
                continue;
            };
            match input.images.get(id) {
                Some(image)
                    if image.size() == texture_size(old)
                        && image.texture_descriptor.format == old.texture.format()
                        && image.data.is_some() =>
                {
                    write(gpu, &old.texture, image.data.as_deref().unwrap_or_default());
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
        input: &mut UiInput,
    ) {
        self.update_images(gpu, input);
        let (origin, size) = (
            Vec2::new(viewport.x as f32, viewport.y as f32),
            Vec2::new(viewport.z.max(1) as f32, viewport.w.max(1) as f32),
        );
        // The rectangles that can be drawn (their pictures exist), with their
        // scissor rectangles in the target.
        let quads = input.quads();
        let mut runs: Vec<(Option<AssetId<Image>>, URect, std::ops::Range<u32>)> = Vec::new();
        let mut data: Vec<f32> = Vec::with_capacity(quads.len() * QUAD_FLOATS);
        for q in &quads {
            let mut rect = q.rect;
            if let Some(id) = q.image {
                let Some(image) = self.image(gpu, id, &input.images) else {
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
