//! Our own 3D scene renderer, drawn with a small wgpu pipeline inside the 2D
//! camera's render schedule. Bevy's built-in 3D pipeline is not used: it
//! renders nothing on some GPUs (observed on Qualcomm Adreno X1-85, DX12 and
//! Vulkan, Bevy 0.18 and 0.19), while plain wgpu works there.
//!
//! The main world fills [`SceneContent`] and [`SceneCamera`]; both are
//! extracted to the render world, uploaded, and drawn before the 2D main pass
//! so that sprites and UI land on top.

use std::sync::Arc;

use bevy::core_pipeline::Core2dSystems;
use bevy::core_pipeline::schedule::Core2d;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderSystems};
use wgpu::util::DeviceExt;

/// Floats per vertex: position (3), uv (2), brightness (1), cutout flag (1),
/// billboard offset (2), animation frame count and uv stride (2).
pub const VERTEX_FLOATS: usize = 11;

/// What the scene shows. `version` changes whenever the content does; the
/// renderer re-uploads only then.
#[derive(Resource, Clone, ExtractResource, Default)]
pub struct SceneContent {
    pub version: u64,
    pub data: Option<Arc<SceneData>>,
}

/// CPU-side scene content.
#[derive(Default)]
pub struct SceneData {
    /// Geometry, drawn in order.
    pub layers: Vec<SceneLayer>,
    /// Panoramic backdrop drawn above the horizon, if any.
    pub sky: Option<RgbaImage>,
    /// Texture for the per-frame sprites in [`SceneSprites`].
    pub sprite_atlas: Option<RgbaImage>,
}

/// Geometry rebuilt every frame (moving sprites such as lemmings), textured
/// with [`SceneData::sprite_atlas`].
#[derive(Resource, Clone, ExtractResource, Default)]
pub struct SceneSprites {
    /// Interleaved vertices, [`VERTEX_FLOATS`] per vertex.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

/// Triangles sharing one texture.
pub struct SceneLayer {
    /// Interleaved vertices, [`VERTEX_FLOATS`] per vertex.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub texture: RgbaImage,
}

/// RGBA8 sRGB texels, row-major.
#[derive(Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub texels: Vec<u8>,
}

/// Per-frame view parameters.
#[derive(Resource, Clone, Copy, ExtractResource, Default)]
pub struct SceneCamera {
    /// World to clip space, wgpu conventions (depth 0..1).
    pub view_proj: Mat4,
    /// Sky texel column shown at the left edge of the screen.
    pub sky_column: f32,
    /// Horizon height as a fraction of the screen from the top.
    pub horizon: f32,
    /// Seconds since start, for animated textures.
    pub time: f32,
    /// Camera right vector; billboards face the camera along it.
    pub right: Vec3,
}

pub struct SceneRenderPlugin;

impl Plugin for SceneRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneContent>()
            .init_resource::<SceneCamera>()
            .init_resource::<SceneSprites>()
            .add_plugins((
                ExtractResourcePlugin::<SceneContent>::default(),
                ExtractResourcePlugin::<SceneCamera>::default(),
                ExtractResourcePlugin::<SceneSprites>::default(),
            ));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .add_systems(Render, prepare_scene.in_set(RenderSystems::Prepare))
            .add_systems(Core2d, draw_scene.before(Core2dSystems::MainPass));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.init_resource::<SceneGpu>();
    }
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const UNIFORM_SIZE: u64 = 96;

/// GPU resources of the scene renderer.
#[derive(Resource)]
struct SceneGpu {
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    /// Geometry and sky pipelines and the colour format they were built for.
    pipelines: Option<(wgpu::TextureFormat, wgpu::RenderPipeline, wgpu::RenderPipeline)>,
    /// Version of the uploaded [`SceneContent`].
    version: Option<u64>,
    layers: Vec<UploadedLayer>,
    sky: Option<wgpu::BindGroup>,
    sprite_atlas: Option<wgpu::BindGroup>,
    /// Per-frame sprite geometry: buffers (grown as needed) and index count.
    sprites: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    /// Depth buffer and its size.
    depth: Option<((u32, u32), wgpu::TextureView)>,
    /// Size of the colour target, for the sky parameters.
    target_size: (u32, u32),
}

struct UploadedLayer {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    bind_group: wgpu::BindGroup,
}

impl FromWorld for SceneGpu {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>().wgpu_device();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SceneGpu {
            layout,
            shader,
            sampler,
            uniforms,
            pipelines: None,
            version: None,
            layers: Vec::new(),
            sky: None,
            sprite_atlas: None,
            sprites: None,
            depth: None,
            target_size: (1, 1),
        }
    }
}

impl SceneGpu {
    fn ensure_pipelines(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.as_ref().is_some_and(|(f, ..)| *f == format) {
            return;
        }
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&self.layout)],
            ..Default::default()
        });
        let geometry = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene geometry"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: (VERTEX_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x2
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene sky"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines = Some((format, geometry, sky));
    }

    fn depth_view(&mut self, device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        if self.depth.as_ref().is_none_or(|(s, _)| *s != size) {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene depth"),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            self.depth = Some((size, view));
        }
        self.depth.as_ref().unwrap().1.clone()
    }

    fn bind_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue, img: &RgbaImage) -> wgpu::BindGroup {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("scene texture"),
                size: wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &img.texels,
        );
        let view = texture.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }
}

fn prepare_scene(
    content: Res<SceneContent>,
    sprites: Res<SceneSprites>,
    camera: Res<SceneCamera>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut gpu: ResMut<SceneGpu>,
) {
    let device = device.wgpu_device();
    let (w, h) = gpu.target_size;
    let scale = h as f32 / 480.0;
    let mut uniforms = camera.view_proj.to_cols_array().to_vec();
    // The sky's left-edge column is given for a 640-wide screen; wider
    // screens show more sky around the same centre.
    let extra = (w as f32 / scale - 640.0) / 2.0;
    uniforms.extend([camera.sky_column - extra / 2.0, camera.horizon * h as f32, scale, camera.time]);
    uniforms.extend([camera.right.x, camera.right.y, camera.right.z, 0.0]);
    queue.write_buffer(&gpu.uniforms, 0, &f32_bytes(&uniforms));
    upload_sprites(device, &queue, &mut gpu, &sprites);
    if gpu.version == Some(content.version) {
        return;
    }
    gpu.version = Some(content.version);
    let Some(data) = content.data.as_ref() else {
        gpu.layers.clear();
        gpu.sky = None;
        gpu.sprite_atlas = None;
        return;
    };
    let layers = data
        .layers
        .iter()
        .filter(|l| !l.indices.is_empty())
        .map(|l| UploadedLayer {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene vertices"),
                contents: &f32_bytes(&l.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene indices"),
                contents: &l.indices.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<_>>(),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: l.indices.len() as u32,
            bind_group: gpu.bind_texture(device, &queue, &l.texture),
        })
        .collect();
    gpu.layers = layers;
    gpu.sky = data.sky.as_ref().map(|s| gpu.bind_texture(device, &queue, s));
    gpu.sprite_atlas = data.sprite_atlas.as_ref().map(|s| gpu.bind_texture(device, &queue, s));
}

/// Copies this frame's sprite geometry into GPU buffers, growing them when
/// they are too small.
fn upload_sprites(device: &wgpu::Device, queue: &wgpu::Queue, gpu: &mut SceneGpu, sprites: &SceneSprites) {
    let count = sprites.indices.len() as u32;
    if count == 0 {
        if let Some(s) = &mut gpu.sprites {
            s.2 = 0;
        }
        return;
    }
    let vbytes = f32_bytes(&sprites.vertices);
    let ibytes: Vec<u8> = sprites.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let fits = gpu
        .sprites
        .as_ref()
        .is_some_and(|(v, i, _)| v.size() >= vbytes.len() as u64 && i.size() >= ibytes.len() as u64);
    if !fits {
        let alloc = |len: usize, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scene sprites"),
                size: (len as u64 * 2).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        gpu.sprites = Some((alloc(vbytes.len(), wgpu::BufferUsages::VERTEX), alloc(ibytes.len(), wgpu::BufferUsages::INDEX), 0));
    }
    let (v, i, n) = gpu.sprites.as_mut().unwrap();
    queue.write_buffer(v, 0, &vbytes);
    queue.write_buffer(i, 0, &ibytes);
    *n = count;
}

fn draw_scene(view: ViewQuery<&ViewTarget>, mut gpu: ResMut<SceneGpu>, mut ctx: RenderContext) {
    let target = view.into_inner();
    let device = ctx.render_device().wgpu_device().clone();
    let size = target.main_texture().size();
    gpu.target_size = (size.width, size.height);
    let depth = gpu.depth_view(&device, (size.width, size.height));
    gpu.ensure_pipelines(&device, target.main_texture_format());
    let gpu = &*gpu;
    if gpu.layers.is_empty() && gpu.sky.is_none() && gpu.sprites.is_none() {
        return;
    }
    let (_, geometry, sky_pipeline) = gpu.pipelines.as_ref().unwrap();
    let color = target.get_color_attachment();
    let encoder = ctx.command_encoder();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(color)],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
            stencil_ops: None,
        }),
        ..Default::default()
    });
    if let Some(sky) = &gpu.sky {
        pass.set_pipeline(sky_pipeline);
        pass.set_bind_group(0, sky, &[]);
        pass.draw(0..3, 0..1);
    }
    pass.set_pipeline(geometry);
    for layer in &gpu.layers {
        pass.set_bind_group(0, &layer.bind_group, &[]);
        pass.set_vertex_buffer(0, layer.vertices.slice(..));
        pass.set_index_buffer(layer.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..layer.index_count, 0, 0..1);
    }
    if let (Some(atlas), Some((v, i, n))) = (&gpu.sprite_atlas, &gpu.sprites)
        && *n > 0
    {
        pass.set_bind_group(0, atlas, &[]);
        pass.set_vertex_buffer(0, v.slice(..));
        pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..*n, 0, 0..1);
    }
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
