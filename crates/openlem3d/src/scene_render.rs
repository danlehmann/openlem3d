//! Our own 3D scene renderer, a small wgpu pipeline. (Bevy's 3D pipeline
//! rendered nothing on some GPUs: Qualcomm Adreno X1-85, DX12 and Vulkan,
//! Bevy 0.18 and 0.19.)
//!
//! The game fills [`SceneContent`], [`SceneSprites`] and [`SceneCamera`];
//! each frame (`crate::gpu`) uploads what changed and draws the scene, before
//! the UI.

use std::sync::Arc;

use l3d_engine::prelude::*;
use wgpu::util::DeviceExt;

use crate::gpu::{Gpu, TARGET_FORMAT};

/// Floats per vertex: position (3), uv (2), brightness (1), cutout flag (1),
/// billboard offset (2), animation frame count and uv stride (2), light (3).
pub const VERTEX_FLOATS: usize = 14;

/// What the scene shows. `version` changes whenever the content does; the
/// renderer re-uploads only then.
#[derive(Clone, Default)]
pub struct SceneContent {
    pub version: u64,
    pub data: Option<Arc<SceneData>>,
}

/// CPU-side scene content.
#[derive(Default, Clone)]
pub struct SceneData {
    /// Geometry, drawn in order.
    pub layers: Vec<SceneLayer>,
    /// Panoramic backdrop drawn above the horizon, if any.
    pub sky: Option<RgbaImage>,
    /// The sky is a tiled all-round backdrop covering the whole screen
    /// (space levels) rather than a panorama above the horizon.
    pub sky_surround: bool,
    /// Texture for the per-frame sprites in [`SceneSprites`].
    pub sprite_atlas: Option<RgbaImage>,
    /// The level's lights, kept to light the blocks again when they are
    /// rebuilt (not drawn themselves).
    pub lights: Vec<crate::light::Light>,
}

/// Geometry rebuilt every frame (moving sprites such as lemmings), textured
/// with [`SceneData::sprite_atlas`].
#[derive(Clone, Default)]
pub struct SceneSprites {
    /// Interleaved vertices, [`VERTEX_FLOATS`] per vertex.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

/// Triangles sharing one texture.
#[derive(Clone)]
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
    pub texels: Arc<[u8]>,
}

/// Per-frame view parameters.
#[derive(Clone, Copy, Default)]
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
    /// Roll in radians (anticlockwise on screen) already in `view_proj`;
    /// the sky is turned by it too.
    pub roll: f32,
    /// The size in pixels the scene is rendered at and then stretched over
    /// the screen without smoothing; zero for the screen's own size.
    pub pixels: UVec2,
    /// The colour behind the scene, linear RGBA, for the low-resolution
    /// target (the screen itself is cleared to the camera's colour).
    pub clear: Vec4,
    /// Let glowing texels shine and bloom (`crate::glow`); only when the
    /// scene is drawn at the screen's resolution.
    pub glow: bool,
}

/// What a frame draws of the scene.
pub struct SceneInput<'a> {
    pub content: &'a SceneContent,
    pub sprites: &'a SceneSprites,
    pub camera: &'a SceneCamera,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The scene's colour format while glowing: room above white.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const UNIFORM_SIZE: u64 = 144;
/// How much brighter than its palette colour a fully glowing texel is drawn
/// (ours, tuned by eye).
const GLOW_GAIN: f32 = 3.0;
/// The most halvings of the bloom chain, and the smallest size worth one.
const BLOOM_LEVELS: usize = 6;
const BLOOM_MIN_SIZE: u32 = 8;
/// Sky texels panned per full turn for the 320-wide all-round skies: three
/// repeats of the panorama, close to the 1024 of the other skies and a
/// whole number of repeats, so no turn shows a seam (provisional).
const SURROUND_SKY_TURN: f32 = 960.0;

/// GPU resources of the scene renderer.
pub struct SceneGpu {
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    /// Smooth sampling, for the bloom chain.
    linear: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    /// Pipelines per colour format drawn into.
    pipelines: Vec<(wgpu::TextureFormat, Pipelines)>,
    /// The low-resolution colour target: its size and format, its view, and
    /// the bind group that samples it.
    low: Option<(
        (u32, u32),
        wgpu::TextureFormat,
        wgpu::TextureView,
        wgpu::BindGroup,
    )>,
    /// Version of the uploaded [`SceneContent`].
    version: Option<u64>,
    layers: Vec<UploadedLayer>,
    sky: Option<wgpu::BindGroup>,
    sprite_atlas: Option<wgpu::BindGroup>,
    /// Per-frame sprite geometry: buffers (grown as needed) and index count.
    sprites: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    /// The glowing scene's targets, made anew when the size changes.
    hdr: Option<HdrTargets>,
    /// Depth buffer and its size.
    depth: Option<((u32, u32), wgpu::TextureView)>,
    /// The part of the colour target drawn in (the camera's viewport): its
    /// top-left corner and size, for the sky parameters.
    target_origin: (u32, u32),
    target_size: (u32, u32),
    /// Uploaded textures by their texel data, kept while the content still
    /// uses that data (a terrain change re-uploads geometry only).
    textures: Vec<(Arc<[u8]>, wgpu::BindGroup)>,
}

struct Pipelines {
    geometry: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    /// Copies an offscreen scene onto the screen: stretched, or glowing.
    stretch: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    /// Bloom: the glow above white, halved; a further halving; and a level
    /// blurred back up and added to the one above (or to the scene).
    bloom_first: wgpu::RenderPipeline,
    bloom_down: wgpu::RenderPipeline,
    bloom_up: wgpu::RenderPipeline,
    bloom_add: wgpu::RenderPipeline,
}

/// The scene drawn in [`HDR_FORMAT`], and the bloom chain's levels, each
/// half the size of the one before.
#[derive(Clone)]
struct HdrTargets {
    size: (u32, u32),
    view: wgpu::TextureView,
    /// The scene sampled sharp (copied to the screen) and smooth (bloom).
    sharp: wgpu::BindGroup,
    smooth: wgpu::BindGroup,
    levels: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

struct UploadedLayer {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    bind_group: wgpu::BindGroup,
}

impl SceneGpu {
    pub fn new(device: &wgpu::Device) -> Self {
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
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene bloom"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
            linear,
            uniforms,
            pipelines: Vec::new(),
            low: None,
            version: None,
            layers: Vec::new(),
            sky: None,
            sprite_atlas: None,
            sprites: None,
            hdr: None,
            depth: None,
            target_origin: (0, 0),
            target_size: (1, 1),
            textures: Vec::new(),
        }
    }

    /// The pipelines drawing into `format`, made on first use.
    fn pipelines(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) -> &Pipelines {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[i].1;
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
                        0 => Float32x3, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x2, 5 => Float32x3
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
                depth_compare: Some(wgpu::CompareFunction::Greater),
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
        // A full-screen triangle with no depth, optionally added onto what
        // is there.
        let full_screen = |label, vertex, fragment, add: bool| {
            let blend = add.then_some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = Pipelines {
            geometry,
            sky,
            stretch: full_screen("scene stretch", "vs_sky", "fs_stretch", false),
            present: full_screen("scene present", "vs_sky", "fs_present", false),
            bloom_first: full_screen("bloom first", "vs_post", "fs_bloom_first", false),
            bloom_down: full_screen("bloom down", "vs_post", "fs_bloom_down", false),
            bloom_up: full_screen("bloom up", "vs_post", "fs_bloom_up", true),
            bloom_add: full_screen("bloom add", "vs_post", "fs_bloom_add", true),
        };
        self.pipelines.push((format, pipelines));
        &self.pipelines.last().unwrap().1
    }

    /// A bind group sampling `view` with `sampler`.
    fn bind_view(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene target"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// The glowing scene's targets for a view of `size`.
    fn hdr_targets(&mut self, device: &wgpu::Device, size: (u32, u32)) -> HdrTargets {
        if self.hdr.as_ref().is_none_or(|h| h.size != size) {
            let target = |label, (w, h): (u32, u32)| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: HDR_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let view = target("scene hdr", size);
            let mut levels = Vec::new();
            let mut s = size;
            while levels.len() < BLOOM_LEVELS && s.0.min(s.1) / 2 >= BLOOM_MIN_SIZE {
                s = (s.0 / 2, s.1 / 2);
                let v = target("scene bloom", s);
                let b = self.bind_view(device, &v, &self.linear);
                levels.push((v, b));
            }
            self.hdr = Some(HdrTargets {
                size,
                sharp: self.bind_view(device, &view, &self.sampler),
                smooth: self.bind_view(device, &view, &self.linear),
                view,
                levels,
            });
        }
        self.hdr.clone().unwrap()
    }

    /// The low-resolution colour target of `size` in `format`, made anew
    /// when either changes.
    fn low_target(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> (wgpu::TextureView, wgpu::BindGroup) {
        if self
            .low
            .as_ref()
            .is_none_or(|(s, f, ..)| *s != size || *f != format)
        {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene low resolution"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let bind = self.bind_view(device, &view, &self.sampler);
            self.low = Some((size, format, view, bind));
        }
        let (_, _, view, bind) = self.low.as_ref().unwrap();
        (view.clone(), bind.clone())
    }

    fn depth_view(&mut self, device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        if self.depth.as_ref().is_none_or(|(s, _)| *s != size) {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene depth"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
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

    fn bind_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &RgbaImage,
    ) -> wgpu::BindGroup {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("scene texture"),
                size: wgpu::Extent3d {
                    width: img.width,
                    height: img.height,
                    depth_or_array_layers: 1,
                },
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// Uploads the uniforms, this frame's sprites and, when it changed, the
/// content.
fn prepare(
    gpu: &mut SceneGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    content: &SceneContent,
    sprites: &SceneSprites,
    camera: &SceneCamera,
) {
    let (w, h) = gpu.target_size;
    let scale = h as f32 / 480.0;
    let mut uniforms = camera.view_proj.to_cols_array().to_vec();
    // The sky's left-edge column is given for a 640-wide screen; wider
    // screens show more sky around the same centre.
    let extra = (w as f32 / scale - 640.0) / 2.0;
    let surround = content.data.as_ref().is_some_and(|d| d.sky_surround);
    let column = if surround {
        camera.sky_column * SURROUND_SKY_TURN / 1024.0
    } else {
        camera.sky_column
    };
    uniforms.extend([
        column - extra / 2.0,
        camera.horizon * h as f32,
        scale,
        camera.time,
    ]);
    uniforms.extend([
        camera.right.x,
        camera.right.y,
        camera.right.z,
        if surround { 1.0 } else { 0.0 },
    ]);
    // Screen pixels per rendered pixel, across and down: the sky is laid out
    // in screen pixels whatever the scene is rendered at.
    let pixel = if camera.pixels.x > 0 && camera.pixels.y > 0 {
        Vec2::new(
            w as f32 / camera.pixels.x as f32,
            h as f32 / camera.pixels.y as f32,
        )
    } else {
        Vec2::ONE
    };
    uniforms.extend([camera.roll, w as f32 / 2.0, pixel.x, pixel.y]);
    // The viewport's corner in the target, for shaders working in framebuffer
    // pixels: the sky's (none when rendered small), then the stretch's.
    let (x, y) = (gpu.target_origin.0 as f32, gpu.target_origin.1 as f32);
    let small = renders_small(camera.pixels, (w, h));
    let glow = camera.glow && !small;
    let sky = if small || glow { [0.0, 0.0] } else { [x, y] };
    uniforms.extend([sky[0], sky[1], x, y]);
    uniforms.extend([if glow { GLOW_GAIN } else { 0.0 }, 0.0, 0.0, 0.0]);
    queue.write_buffer(&gpu.uniforms, 0, &f32_bytes(&uniforms));
    upload_sprites(device, queue, gpu, sprites);
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
    let old = std::mem::take(&mut gpu.textures);
    let bind = |gpu: &mut SceneGpu, img: &RgbaImage| {
        let same = |(t, _): &&(Arc<[u8]>, wgpu::BindGroup)| Arc::ptr_eq(t, &img.texels);
        if let Some((_, b)) = gpu.textures.iter().find(same) {
            return b.clone();
        }
        let b = match old.iter().find(same) {
            Some((_, b)) => b.clone(),
            None => gpu.bind_texture(device, queue, img),
        };
        gpu.textures.push((img.texels.clone(), b.clone()));
        b
    };
    let mut layers = Vec::new();
    for l in data.layers.iter().filter(|l| !l.indices.is_empty()) {
        let bind_group = bind(gpu, &l.texture);
        layers.push(UploadedLayer {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene vertices"),
                contents: &f32_bytes(&l.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene indices"),
                contents: &l
                    .indices
                    .iter()
                    .flat_map(|i| i.to_le_bytes())
                    .collect::<Vec<_>>(),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: l.indices.len() as u32,
            bind_group,
        });
    }
    gpu.layers = layers;
    gpu.sky = data.sky.as_ref().map(|s| bind(gpu, s));
    gpu.sprite_atlas = data.sprite_atlas.as_ref().map(|s| bind(gpu, s));
}

/// Copies this frame's sprite geometry into GPU buffers, growing them when
/// they are too small.
fn upload_sprites(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu: &mut SceneGpu,
    sprites: &SceneSprites,
) {
    let count = sprites.indices.len() as u32;
    if count == 0 {
        if let Some(s) = &mut gpu.sprites {
            s.2 = 0;
        }
        return;
    }
    let vbytes = f32_bytes(&sprites.vertices);
    let ibytes: Vec<u8> = sprites
        .indices
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let fits = gpu.sprites.as_ref().is_some_and(|(v, i, _)| {
        v.size() >= vbytes.len() as u64 && i.size() >= ibytes.len() as u64
    });
    if !fits {
        let alloc = |len: usize, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scene sprites"),
                size: (len as u64 * 2).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        gpu.sprites = Some((
            alloc(vbytes.len(), wgpu::BufferUsages::VERTEX),
            alloc(ibytes.len(), wgpu::BufferUsages::INDEX),
            0,
        ));
    }
    let (v, i, n) = gpu.sprites.as_mut().unwrap();
    queue.write_buffer(v, 0, &vbytes);
    queue.write_buffer(i, 0, &ibytes);
    *n = count;
}

impl SceneGpu {
    /// Draws the scene into `viewport` (x, y, width, height) of `target`, a
    /// frame of `full` size in [`TARGET_FORMAT`].
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        full: UVec2,
        viewport: UVec4,
        input: &SceneInput,
    ) {
        let size = (viewport.z.max(1), viewport.w.max(1));
        self.target_origin = (viewport.x, viewport.y);
        self.target_size = size;
        let device = &gpu.device;
        prepare(
            self,
            device,
            &gpu.queue,
            input.content,
            input.sprites,
            input.camera,
        );
        draw(self, device, encoder, target, full, viewport, input.camera);
    }
}

fn draw(
    gpu: &mut SceneGpu,
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    full: UVec2,
    viewport: UVec4,
    camera: &SceneCamera,
) {
    let size = gpu.target_size;
    let format = TARGET_FORMAT;
    let device = device.clone();
    // Rendered small and stretched over the screen, or at the screen's size:
    // offscreen in floating point while glowing, else straight onto it.
    let low = renders_small(camera.pixels, size).then(|| {
        let s = (camera.pixels.x, camera.pixels.y);
        (s, gpu.low_target(&device, s, format))
    });
    let hdr = (camera.glow && low.is_none()).then(|| gpu.hdr_targets(&device, size));
    let scene_format = if hdr.is_some() { HDR_FORMAT } else { format };
    gpu.pipelines(&device, scene_format);
    gpu.pipelines(&device, HDR_FORMAT);
    gpu.pipelines(&device, format);
    let depth = gpu.depth_view(
        &device,
        match (&low, &hdr) {
            (Some(l), _) => l.0,
            (_, Some(h)) => h.size,
            _ => (full.x, full.y),
        },
    );
    let gpu = &*gpu;
    if gpu.layers.is_empty() && gpu.sky.is_none() && gpu.sprites.is_none() {
        return;
    }
    let find = |f| &gpu.pipelines.iter().find(|(g, _)| *g == f).unwrap().1;
    let (scene_p, hdr_p, screen_p) = (find(scene_format), find(HDR_FORMAT), find(format));
    let clear = wgpu::LoadOp::Clear(wgpu::Color {
        r: camera.clear.x as f64,
        g: camera.clear.y as f64,
        b: camera.clear.z as f64,
        a: camera.clear.w as f64,
    });
    let offscreen = |view| wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: clear,
            store: wgpu::StoreOp::Store,
        },
    };
    // The frame is already cleared to the clear colour.
    let onto_target = || wgpu::RenderPassColorAttachment {
        view: target,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        },
    };
    let color = match (&low, &hdr) {
        (Some((_, (view, _))), _) => offscreen(view),
        (_, Some(h)) => offscreen(&h.view),
        _ => onto_target(),
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(color)],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        ..Default::default()
    });
    if low.is_none() && hdr.is_none() {
        set_viewport(&mut pass, viewport);
    }
    if let Some(sky) = &gpu.sky {
        pass.set_pipeline(&scene_p.sky);
        pass.set_bind_group(0, sky, &[]);
        pass.draw(0..3, 0..1);
    }
    pass.set_pipeline(&scene_p.geometry);
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
    drop(pass);
    // Bloom: halve the glow above white down the chain, then blur each
    // level back up into the one above.
    if let Some(h) = &hdr {
        let mut step = |into: &wgpu::TextureView, load, pipeline, from: &wgpu::BindGroup| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: into,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, from, &[]);
            pass.draw(0..3, 0..1);
        };
        let black = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        for (k, (view, _)) in h.levels.iter().enumerate() {
            match k {
                0 => step(view, black, &hdr_p.bloom_first, &h.smooth),
                _ => step(view, black, &hdr_p.bloom_down, &h.levels[k - 1].1),
            }
        }
        for k in (1..h.levels.len()).rev() {
            step(
                &h.levels[k - 1].0,
                wgpu::LoadOp::Load,
                &hdr_p.bloom_up,
                &h.levels[k].1,
            );
        }
        if let Some((_, first)) = h.levels.first() {
            step(&h.view, wgpu::LoadOp::Load, &hdr_p.bloom_add, first);
        }
    }
    let shown = match (&low, &hdr) {
        (Some((_, (_, bind))), _) => Some((bind, &screen_p.stretch)),
        (_, Some(h)) => Some((&h.sharp, &screen_p.present)),
        _ => None,
    };
    if let Some((bind, pipeline)) = shown {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene stretch"),
            color_attachments: &[Some(onto_target())],
            ..Default::default()
        });
        set_viewport(&mut pass, viewport);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Whether the scene is rendered at `pixels` and stretched over a view of
/// `size`, rather than at the view's own size.
fn renders_small(pixels: UVec2, size: (u32, u32)) -> bool {
    pixels.x > 0 && pixels.y > 0 && (pixels.x < size.0 || pixels.y < size.1)
}

/// Draws into the camera's viewport (x, y, width, height) only.
fn set_viewport(pass: &mut wgpu::RenderPass, viewport: UVec4) {
    pass.set_viewport(
        viewport.x as f32,
        viewport.y as f32,
        viewport.z.max(1) as f32,
        viewport.w.max(1) as f32,
        0.0,
        1.0,
    );
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
