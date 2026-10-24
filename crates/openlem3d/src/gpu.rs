//! The GPU: wgpu's device, the window's surface, and each frame. The scene
//! and then the UI are drawn into a texture the size of the window (sRGB
//! RGBA8, as Bevy's renderer did), which is then copied onto the window and,
//! when asked, saved as a screenshot.
//!
//! The device is made once the window exists. On Android the window goes
//! away while the app is suspended (bevy_winit then takes its handle away)
//! and comes back as a new one, so the surface follows the handle.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use l3d_engine::prelude::*;
use l3d_engine::window::{PresentMode, WindowHandle};

use crate::World;
use crate::profile::{self, Cpu, Stamp};
use crate::scene_render::{SceneGpu, SceneInput};
use crate::ui::UiGpu;

/// The colour behind everything, where nothing else is drawn.
#[derive(Clone, Copy)]
pub struct ClearColor(pub Color);

/// Files to save the next frame to, as PNG.
#[derive(Default)]
pub struct Screenshots(pub Vec<PathBuf>);

/// The format everything is drawn in before it is copied to the window.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Default)]
pub(crate) struct Renderer {
    /// The window handle the surface was made for.
    generation: u64,
    gpu: Option<Gpu>,
    surface: Option<Surface>,
    target: Option<Target>,
    copy: Option<Copy>,
    scene: Option<SceneGpu>,
    ui: Option<UiGpu>,
}

pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Set when the driver gives the device up.
    lost: Arc<AtomicBool>,
}

impl Gpu {
    /// The device for the window `handle`, and the window's surface; with
    /// GPU timestamps if `profile` asks for them and the GPU has them, and
    /// with the graphics APIs in `backend` if given.
    fn new(
        handle: &WindowHandle,
        profile: bool,
        backend: Option<wgpu::Backends>,
    ) -> (Gpu, wgpu::Surface<'static>) {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        // Vulkan on Windows-on-ARM (Adreno) loses the device after a few
        // seconds, so Windows defaults to DX12. `WGPU_BACKEND` overrides.
        if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
            desc.backends = wgpu::Backends::DX12;
        }
        if let Some(b) = backend {
            desc.backends = b;
        }
        let instance = wgpu::Instance::new(desc);
        let surface = create_surface(&instance, handle);
        let adapter =
            pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::from_env()
                        .unwrap_or(wgpu::PowerPreference::HighPerformance),
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                }),
            )
            .expect("no GPU found");
        info!("{:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("openlem3d"),
            required_features: if profile {
                adapter.features() & profile::FEATURES
            } else {
                wgpu::Features::empty()
            },
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("could not open the GPU");
        // Logged rather than fatal, as Bevy's renderer did.
        device.on_uncaptured_error(Arc::new(|e| error!("GPU error: {e}")));
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            error!("GPU lost ({reason:?}): {message}");
            flag.store(true, Ordering::Release);
        });
        let gpu = Gpu {
            instance,
            adapter,
            device,
            queue,
            lost,
        };
        (gpu, surface)
    }

    /// Waits for the GPU to finish its work, as configuring a surface needs.
    /// wgpu waits too, once, but the Pixel 11 Pro's PowerVR driver sometimes
    /// ends a wait early (well before its timeout), and wgpu then refuses to
    /// configure the surface; waiting until the work is done avoids that.
    fn idle(&self) {
        const LIMIT: Duration = Duration::from_secs(1);
        let start = Instant::now();
        let wait = wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(LIMIT),
        };
        while let Err(e) = self.device.poll(wait.clone()) {
            if !matches!(e, wgpu::PollError::Timeout) || start.elapsed() >= LIMIT {
                warn!("GPU not idle before configuring the surface: {e}");
                return;
            }
        }
    }
}

fn create_surface(instance: &wgpu::Instance, handle: &WindowHandle) -> wgpu::Surface<'static> {
    instance
        .create_surface(handle.0.clone())
        .expect("could not draw into the window")
}

/// The window's surface and how it is configured.
struct Surface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The format of the views drawn into it: an sRGB one.
    view_format: wgpu::TextureFormat,
    /// Configured for a window that has since changed.
    stale: bool,
}

impl Surface {
    /// Bevy's choice: sRGB RGBA8 or BGRA8 where the window offers it.
    fn new(gpu: &Gpu, surface: wgpu::Surface<'static>) -> Self {
        let formats = surface.get_capabilities(&gpu.adapter).formats;
        let format = formats
            .iter()
            .copied()
            .find(|f| {
                *f == wgpu::TextureFormat::Rgba8UnormSrgb
                    || *f == wgpu::TextureFormat::Bgra8UnormSrgb
            })
            .or(formats.first().copied())
            .expect("the window offers no formats");
        let view_format = format.add_srgb_suffix();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 0,
            height: 0,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: if view_format == format {
                Vec::new()
            } else {
                vec![view_format]
            },
        };
        Surface {
            surface,
            config,
            view_format,
            stale: true,
        }
    }

    /// Configures it for the window's size and present mode, if that changed.
    fn update(&mut self, gpu: &Gpu, size: UVec2, present: PresentMode) {
        let present = match present {
            PresentMode::AutoVsync => wgpu::PresentMode::AutoVsync,
            PresentMode::AutoNoVsync => wgpu::PresentMode::AutoNoVsync,
            PresentMode::Fifo => wgpu::PresentMode::Fifo,
            PresentMode::FifoRelaxed => wgpu::PresentMode::FifoRelaxed,
            PresentMode::Immediate => wgpu::PresentMode::Immediate,
            PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        };
        let c = &mut self.config;
        if !self.stale && c.width == size.x && c.height == size.y && c.present_mode == present {
            return;
        }
        (c.width, c.height, c.present_mode) = (size.x, size.y, present);
        gpu.idle();
        self.surface.configure(&gpu.device, &self.config);
        self.stale = false;
    }
}

/// The texture a frame is drawn into.
struct Target {
    size: UVec2,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Samples it, for copying it to the window.
    bind_group: wgpu::BindGroup,
}

/// Copies the target onto the window: a triangle covering it, sampled
/// texel for texel.
struct Copy {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    shader: wgpu::ShaderModule,
    /// Pipelines per window format.
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

const COPY_SHADER: &str = "
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: Out;
    out.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    return textureSample(frame, frame_sampler, in.uv);
}
";

impl Copy {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("copy"),
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
        Copy {
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor::default()),
            shader: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("copy"),
                source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
            }),
            pipelines: Vec::new(),
        }
    }

    /// The pipeline drawing into `format`, made on first use.
    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[i].1;
        }
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("copy"),
            bind_group_layouts: &[Some(&self.layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("copy"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, pipeline));
        &self.pipelines.last().expect("just pushed").1
    }

    fn target(&self, device: &wgpu::Device, size: UVec2) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("copy"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Target {
            size,
            texture,
            view,
            bind_group,
        }
    }
}

/// Draws a frame: the clear colour, the scene in the screen's part of the
/// window, the UI, then onto the window.
pub(crate) fn render(w: &mut World) {
    let Renderer {
        generation,
        gpu,
        surface,
        target,
        copy,
        scene,
        ui,
    } = &mut w.renderer;
    let (window, screen, clear, screenshots) =
        (&w.e.window, &w.screen, &w.clear, &mut w.screenshots);
    let profile = &mut w.profile;
    let backend = w.opts.backend;
    let Some(handle) = &w.e.handle else {
        // The app is suspended (Android): the window is gone.
        *surface = None;
        return;
    };
    let new_handle = *generation != w.e.handle_generation;
    *generation = w.e.handle_generation;
    match gpu {
        None => {
            let (mut g, mut s) = Gpu::new(handle, profile.on, backend);
            // Old Vulkan drivers can fail to compile a shader and give the
            // device up (the Nexus 6P's Adreno 430), where their OpenGL ES
            // works. Compiling before the window is attached lets OpenGL ES
            // take over cleanly.
            let mut sc = SceneGpu::new(&g.device, &g.adapter);
            sc.compile(&g.device);
            if g.lost.load(Ordering::Acquire)
                && g.adapter.get_info().backend == wgpu::Backend::Vulkan
            {
                warn!("the Vulkan driver failed; using OpenGL ES");
                drop((sc, s, g));
                (g, s) = Gpu::new(handle, profile.on, Some(wgpu::Backends::GL));
                sc = SceneGpu::new(&g.device, &g.adapter);
            }
            *scene = Some(sc);
            profile.start_gpu(&g.device, &g.queue);
            *surface = Some(Surface::new(&g, s));
            *gpu = Some(g);
        }
        Some(g) if new_handle || surface.is_none() => {
            *surface = Some(Surface::new(g, create_surface(&g.instance, handle)));
        }
        Some(_) => {}
    }
    let (Some(gpu), Some(s)) = (gpu.as_ref(), surface.as_mut()) else {
        return;
    };
    let size = UVec2::new(window.physical_width(), window.physical_height()).max(UVec2::ONE);
    s.update(gpu, size, window.present_mode);
    // Without a frame to show (the window hidden, say), a screenshot is
    // still drawn.
    let mut hidden = false;
    let frame = match s.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) => Some(t),
        wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
            s.stale = true;
            Some(t)
        }
        wgpu::CurrentSurfaceTexture::Timeout => None,
        wgpu::CurrentSurfaceTexture::Occluded => {
            hidden = true;
            None
        }
        wgpu::CurrentSurfaceTexture::Outdated => {
            s.stale = true;
            None
        }
        other => {
            warn!("lost the window's surface ({other:?}); making it again");
            *surface = None;
            return;
        }
    };
    profile.cpu(Cpu::Acquire);
    if frame.is_none() && screenshots.0.is_empty() {
        // A hidden window (covered, minimised, the screen locked) gives no
        // image and no wait for one: rest instead of spinning a core.
        if hidden {
            std::thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    profile.collect(&gpu.device);
    let device = &gpu.device;
    let copy = copy.get_or_insert_with(|| Copy::new(device));
    if target.as_ref().is_none_or(|t| t.size != size) {
        *target = Some(copy.target(device, size));
    }
    let target = target.as_ref().expect("made above");
    let mut encoder = device.create_command_encoder(&Default::default());
    profile.stamp(&mut encoder, Stamp::Start);
    let c = clear.0.to_linear();
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: c.red as f64,
                    g: c.green as f64,
                    b: c.blue as f64,
                    a: c.alpha as f64,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    profile.stamp(&mut encoder, Stamp::Clear);
    let viewport = screen.pixels;
    scene
        .get_or_insert_with(|| SceneGpu::new(device, &gpu.adapter))
        .draw(
            gpu,
            &mut encoder,
            &target.view,
            size,
            viewport,
            &SceneInput {
                content: &w.scene,
                sprites: &w.sprites,
                camera: &w.camera,
                profile,
            },
        );
    profile.stamp(&mut encoder, Stamp::Scene);
    ui.get_or_insert_with(|| UiGpu::new(gpu)).draw(
        gpu,
        &mut encoder,
        &target.view,
        viewport,
        &w.ui,
        &mut w.e.images,
    );
    profile.stamp(&mut encoder, Stamp::Ui);
    let s = surface.as_ref().expect("checked above");
    if let Some(frame) = &frame {
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(s.view_format),
            ..Default::default()
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("copy"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(copy.pipeline(device, s.view_format));
            pass.set_bind_group(0, &target.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
    profile.stamp(&mut encoder, Stamp::Copy);
    profile.resolve(&mut encoder);
    let shot = (!screenshots.0.is_empty()).then(|| read_back(device, &mut encoder, target));
    profile.cpu(Cpu::Encode);
    gpu.queue.submit([encoder.finish()]);
    profile.submitted();
    if let Some(frame) = frame {
        frame.present();
    }
    profile.cpu(Cpu::Present);
    if let Some(buffer) = shot {
        save(device, &buffer, size, &std::mem::take(&mut screenshots.0));
    }
}

/// Bytes per row of a screenshot's copy, as wgpu wants them aligned.
fn padded_row(width: u32) -> u32 {
    (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}

/// Copies the frame into a buffer to read.
fn read_back(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &Target,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot"),
        size: (padded_row(target.size.x) * target.size.y) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row(target.size.x)),
                rows_per_image: None,
            },
        },
        target.texture.size(),
    );
    buffer
}

/// Saves the frame read back into `buffer` to each of `paths`.
fn save(device: &wgpu::Device, buffer: &wgpu::Buffer, size: UVec2, paths: &[PathBuf]) {
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    if let Err(e) = device.poll(wgpu::PollType::wait_indefinitely()) {
        error!("could not read the screenshot back: {e}");
        return;
    }
    let row = padded_row(size.x) as usize;
    let texels: Vec<u8> = slice
        .get_mapped_range()
        .chunks(row)
        .flat_map(|r| r[..size.x as usize * 4].iter().copied())
        .collect();
    // RGB: the frame is opaque.
    let rgb: Vec<u8> = texels
        .chunks(4)
        .flat_map(|t| t[..3].iter().copied())
        .collect();
    for path in paths {
        match save_png(path, size, &rgb) {
            Ok(()) => info!("Screenshot saved to {}", path.display()),
            Err(e) => error!("could not save a screenshot to {}: {e}", path.display()),
        }
    }
}

fn save_png(path: &std::path::Path, size: UVec2, rgb: &[u8]) -> Result<(), png::EncodingError> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, size.x, size.y);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgb)
}
