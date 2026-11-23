//! Our own 3D scene renderer, a small wgpu pipeline. (Bevy's 3D pipeline
//! rendered nothing on some GPUs: Qualcomm Adreno X1-85, DX12 and Vulkan,
//! Bevy 0.18 and 0.19.)
//!
//! The game fills [`SceneContent`], [`SceneSprites`] and [`SceneCamera`];
//! each frame (`crate::gpu`) uploads what changed and draws the scene, before
//! the UI.

use std::collections::HashMap;
use std::sync::Arc;

use l3d_engine::prelude::*;
use wgpu::util::DeviceExt;

use crate::gpu::Gpu;
use crate::profile::Stamp;

/// Floats per vertex: position (3), uv (2), shade (1), cutout flag (1),
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
    /// The sky shows only sky (stars, planets, clouds): an all-round
    /// backdrop, or a panorama with no scenery along its horizon.
    pub sky_bare: bool,
    /// The all-round sky is a night sky of stars (ours): its stars twinkle
    /// and shine above white on an HDR screen ([`SceneCamera::ours`]).
    pub starry: bool,
    /// Texture for the per-frame sprites in [`SceneSprites`].
    pub sprite_atlas: Option<RgbaImage>,
    /// The level's lights, kept to light the blocks again when they are
    /// rebuilt (not drawn themselves).
    pub lights: Vec<crate::light::Light>,
    /// The bottom layer's blocks show their sides and undersides too, not
    /// only their tops (see [`crate::scene_build::Show::bottom_sides`]).
    pub bottom_sides: bool,
    /// The height of a sea of water that mirrors the level (ours, enhanced
    /// mode), if any.
    pub mirror: Option<f32>,
    /// A sandstorm blows over the level (ours, Ultra mode).
    pub storm: bool,
    /// The sea has waves (ours; the scene shader's `SEA`).
    pub waves: bool,
    /// The area around the level whose waves are worked out ahead each
    /// frame (`WaveField`): its corner's x and z and its size, in units.
    pub wave_area: Option<[f32; 3]>,
    /// Snow falls gently over the level (ours, Enhanced and Ultra modes).
    pub snow: bool,
    /// The lemmings leave marks in the snow and on the ice (ours, Ultra
    /// mode; see `crate::marks`).
    pub marks: bool,
    /// What faces up at the height of [`Self::mirror`] shines with the level
    /// mirrored there (ours, Ultra mode), this much (1 for a polished floor;
    /// 0 for none).
    pub glossy: f32,
    /// Only the tops of slippery blocks shine (`glossy`), not the rest of
    /// what faces up there.
    pub glossy_slippery: bool,
    /// The land is a circuit board whose traces carry signals the lemmings
    /// set off (ours; see `crate::signals`).
    pub circuit: Option<crate::signals::CircuitBoard>,
    /// The block texture's windows look out on space (ours; their glass
    /// marked, `scene_build::WINDOW_ALPHA`).
    pub windows: bool,
    /// On a level with a sandstorm or windows onto space, the grid cells
    /// holding a visible block, a bit each: [`SOLID_WORDS`] words, word `y · 32 + z`, bit `x`
    /// (`scene.wgsl`'s `Uniforms::solid`).
    pub solid: Option<Arc<[u32]>>,
    /// Which texture-set tiles show snow, by tile, on a level with snow
    /// lying on its blocks (ours; `scene_build::SNOW_DEPTH`).
    pub snow_tiles: Option<Arc<[bool]>>,
}

/// Geometry rebuilt every frame (moving sprites such as lemmings), textured
/// with [`SceneData::sprite_atlas`].
#[derive(Clone, Default)]
pub struct SceneSprites {
    /// Interleaved vertices, [`VERTEX_FLOATS`] per vertex.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    /// Soft round shadows darkening what is under them (ours, enhanced
    /// mode), in the same layout: uv runs from −1 to 1 across each disc,
    /// whose middle is darkened by the shade (0–1).
    pub shadow_vertices: Vec<f32>,
    pub shadow_indices: Vec<u32>,
}

/// Triangles sharing one texture.
#[derive(Clone)]
pub struct SceneLayer {
    /// Interleaved vertices, [`VERTEX_FLOATS`] per vertex.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub texture: RgbaImage,
    /// The texture has mipmaps and, with [`SceneCamera::filtered`], is
    /// sampled trilinearly and anisotropically when shrunk (the sea and the
    /// land, seen far off).
    pub smooth: bool,
    /// The layer shows the last frame drawn, the whole screen stretched
    /// over uv 0–1 (top left to bottom right), in place of `texture`,
    /// which stands in until there is one.
    pub screen: bool,
    /// What the layer is, which decides the scene shader's features it is
    /// drawn with ([`Features::for_layer`]).
    pub kind: LayerKind,
}

/// What a scene layer is, for the features its pixels can show.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LayerKind {
    /// The level's blocks (and builders' bricks): windows, snow, the
    /// circuit board, polished floors.
    #[default]
    Blocks,
    /// A screen showing the game: as a cathode-ray tube.
    Screen,
    /// The sea: waves and surf.
    Sea,
    /// The land, and other ground planes: polished, marked in snow,
    /// sand drifting over it, a circuit board's signals.
    Land,
    /// Objects, traps and signs, and the sprites: none of them.
    Objects,
    /// Snow lying on the blocks.
    Snow,
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
    /// Let glowing texels shine (`crate::glow`), and light what is near
    /// them.
    pub glow: bool,
    /// With `glow`, let it bloom softly round them too (drawing offscreen in
    /// floating point, then blurring); only when the scene is drawn at the
    /// screen's resolution. Without, glowing texels clip at white.
    pub bloom: bool,
    /// Sample [`SceneLayer::smooth`] textures through their mipmaps, and
    /// with `anisotropic`, anisotropically too.
    pub filtered: bool,
    pub anisotropic: bool,
    /// Draw [`SceneLayer::screen`] layers as the glass of a cathode-ray
    /// tube (ours).
    pub crt: bool,
    /// Our touches beyond the original show (Enhanced and Ultra modes).
    pub ours: bool,
    /// How enclosed the camera is, 0 (open air) to 1 (inside a tunnel):
    /// the dust sweeping past it and the snow falling stay outside.
    pub shelter: f32,
}

/// Draws over a pass's drawing (the UI): given the pass, and whether it has
/// a depth buffer.
pub type Overlay<'a> = dyn Fn(&mut wgpu::RenderPass<'_>, bool) + 'a;

/// What a frame draws of the scene.
pub struct SceneInput<'a> {
    pub content: &'a SceneContent,
    pub sprites: &'a SceneSprites,
    pub camera: &'a SceneCamera,
    /// Times the scene's parts on the GPU (`--profile`).
    pub profile: &'a crate::profile::Profile,
    /// The format of the target drawn into.
    pub format: wgpu::TextureFormat,
    /// How far above white the screen shows (an HDR target), or 0 (SDR:
    /// glowing things clip at white).
    pub headroom: f32,
    /// The last frame drawn, for [`SceneLayer::screen`] layers.
    pub screen: Option<&'a wgpu::TextureView>,
    /// The circuit board's signal field, if the level has one, and the part of
    /// it changed since the last frame.
    pub signals: Option<(&'a crate::signals::SignalField, Option<[u32; 4]>)>,
    /// The marks in snow, if the level shows them, and the part of them
    /// changed since the last frame.
    pub marks: Option<(&'a crate::marks::MarkField, Option<[u32; 4]>)>,
    /// The colour the target is cleared to by the first pass drawing into
    /// it (rather than by a pass of its own: on a tiling GPU each pass over
    /// the whole frame costs).
    pub clear: wgpu::Color,
    /// Drawn over the scene in the last pass onto the target (the UI), so
    /// that a tiling GPU needn't write the frame out and read it back for
    /// it.
    /// Told whether the pass has the scene's depth buffer
    /// ([`SceneGpu::depth_format`]).
    pub overlay: Option<&'a Overlay<'a>>,
}

/// Which field the level's field texture holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Signals,
    Marks,
}

/// The scene's colour format while glowing: room above white.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const UNIFORM_SIZE: u64 = 288 + SOLID_WORDS as u64 * 4;
/// Words of [`SceneData::solid`]: a bit per cell of the 32 × 16 × 32 grid.
pub const SOLID_WORDS: usize = 16 * 32;
/// Snowflakes (`scene.wgsl`'s `SNOW_FLAKES`).
const SNOW_FLAKES: u32 = 3000;
/// Sand grains (`scene.wgsl`'s `SAND_GRAINS`).
const SAND_GRAINS: u32 = 4000;
/// How much brighter than its palette colour a fully glowing texel is drawn
/// (ours, tuned by eye).
const GLOW_GAIN: f32 = 3.0;
/// The most halvings of the bloom chain, and the smallest size worth one.
const BLOOM_LEVELS: usize = 6;
const BLOOM_MIN_SIZE: u32 = 8;
/// The bloom chain's top level is at most this many rows (ours): on a
/// taller screen it starts at a quarter of the size or less, as a blur needs
/// no more detail, which spares slower GPUs the reading and writing of a
/// big level (a third of the frame's GPU time at 2880×1440 on an Adreno 540).
const BLOOM_TOP_ROWS: u32 = 540;
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
    /// Compiled pipelines kept between runs (where the driver can: Vulkan),
    /// and the file they are kept in.
    cache: Option<(wgpu::PipelineCache, std::path::PathBuf)>,
    /// Trilinear, anisotropic and repeating, for [`SceneLayer::smooth`];
    /// and the same without anisotropy.
    mipmapped: wgpu::Sampler,
    trilinear: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    /// Pipelines per colour format drawn into.
    pipelines: HashMap<(wgpu::TextureFormat, Which, Features), wgpu::RenderPipeline>,
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
    /// The sky, a night sky sampled smoothly in our modes
    /// ([`SceneCamera::filtered`]).
    sky: Option<Bound>,
    sprite_atlas: Option<wgpu::BindGroup>,
    /// Per-frame sprite geometry: buffers (grown as needed) and index count.
    sprites: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    /// The same for the shadows.
    shadows: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    /// The glowing scene's targets, made anew when the size changes.
    hdr: Option<HdrTargets>,
    /// Depth buffer and its size.
    depth: Option<((u32, u32), wgpu::TextureView)>,
    /// The level mirrored in the sea: its size and format, and the picture
    /// and its bind group (sampled smooth), made anew when either changes.
    reflection: Option<(
        (u32, u32),
        wgpu::TextureFormat,
        wgpu::TextureView,
        wgpu::BindGroup,
    )>,
    /// The waves worked out ahead for the area around the level, each
    /// frame (`WaveField`).
    wave_field: Option<wgpu::TextureView>,
    /// A one-texel picture bound in place of a picture there is not: as a
    /// bind group in the second place, and as a view.
    blank: wgpu::BindGroup,
    blank_view: wgpu::TextureView,
    /// The scene's second bind group: the reflection (`second_tex`) and the
    /// level's field (`field_tex`), either blank.
    extras_layout: wgpu::BindGroupLayout,
    /// The level's field on the GPU, a circuit board's signals or the marks
    /// in snow: which field it is, the texture (RGBA8) and its view.
    field: Option<(Field, u64, wgpu::Texture, wgpu::TextureView)>,
    /// Floats where the GPU can draw into them, for reversed depth's
    /// precision; otherwise (the Nexus 6P's Adreno 430 on Vulkan) 24 bits.
    depth_format: wgpu::TextureFormat,
    /// The part of the colour target drawn in (the camera's viewport): its
    /// top-left corner and size, for the sky parameters.
    target_origin: (u32, u32),
    target_size: (u32, u32),
    /// Uploaded textures by their texel data, kept while the content still
    /// uses that data (a terrain change re-uploads geometry only).
    textures: Vec<(Arc<[u8]>, bool, Bound)>,
}

/// The features a level's scene shows, each an override constant of the
/// scene shader (`scene.wgsl`): its pipelines are compiled with only these,
/// so that no pixel pays for the others (on a weak GPU a large share of
/// the frame).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
struct Features {
    windows: bool,
    screens: bool,
    sea: bool,
    circuit: bool,
    snow: bool,
    gloss: bool,
    storm: bool,
    mirror: bool,
}

impl Features {
    /// Those of the level's features a layer of `kind` can show (a sandstorm's
    /// dust and the sea's mirror apply to every surface).
    fn for_layer(self, kind: LayerKind) -> Features {
        let only = Features {
            storm: self.storm,
            mirror: self.mirror,
            ..Features::default()
        };
        match kind {
            LayerKind::Blocks => Features {
                windows: self.windows,
                circuit: self.circuit,
                snow: self.snow,
                gloss: self.gloss,
                ..only
            },
            LayerKind::Screen => Features {
                screens: self.screens,
                ..only
            },
            LayerKind::Sea => Features {
                sea: self.sea,
                ..only
            },
            // A circuit board is the land (`scene_build::CIRCUIT_LANDS`).
            LayerKind::Land => Features {
                circuit: self.circuit,
                snow: self.snow,
                gloss: self.gloss,
                ..only
            },
            LayerKind::Snow => Features {
                snow: self.snow,
                ..only
            },
            LayerKind::Objects => only,
        }
    }

    fn constants(self) -> Vec<(&'static str, f64)> {
        let on = |b: bool| if b { 1.0 } else { 0.0 };
        vec![
            ("WINDOWS", on(self.windows)),
            ("SCREENS", on(self.screens)),
            ("SEA", on(self.sea)),
            ("CIRCUIT", on(self.circuit)),
            ("SNOW", on(self.snow)),
            ("GLOSS", on(self.gloss)),
            ("STORM", on(self.storm)),
            ("MIRROR", on(self.mirror)),
        ]
    }
}

impl Which {
    /// The features pipeline `self` is made for: the geometry's, the
    /// level's; the rest don't use them.
    fn specialised(self, features: Features) -> Features {
        match self {
            Which::Geometry | Which::Mirrored => features,
            _ => Features::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Which {
    Geometry,
    /// The geometry mirrored in the sea (drawn as its second instance).
    Mirrored,
    Sky,
    /// Copies an offscreen scene onto the screen: stretched, or glowing.
    Stretch,
    Present,
    /// Presenting with the bloom chain's top level blurred up and added in
    /// the same pass, which spares the full-size picture a pass of its own.
    PresentGlow,
    /// Bloom: the glow above white, halved; a further halving; and a level
    /// blurred back up and added to the one above.
    BloomFirst,
    BloomDown,
    BloomUp,
    /// Shadows: multiplied onto what is drawn, behind what is in front.
    Shadow,
    /// Sheets of dust sweeping across the view in a sandstorm.
    DustSheets,
    /// Sand grains blown along the wind in a sandstorm.
    Sand,
    /// Snowflakes falling gently.
    Snow,
    /// The waves worked out ahead into [`WAVE_FIELD_FORMAT`].
    WaveField,
}

/// The waves' heights and slopes for the area around the level are worked
/// out once a frame into a texture of this many texels across, which the
/// sea's pixels and vertices near the camera read (`scene.wgsl`'s
/// `waves_at`): on a weak GPU, working them out for every pixel was most
/// of the frame (a Mali-G31: 125 ms at 720p).
const WAVE_FIELD: u32 = 256;
const WAVE_FIELD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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
    bind_group: Bound,
    /// Shows the last frame drawn ([`SceneLayer::screen`]).
    screen: bool,
    kind: LayerKind,
}

/// An uploaded texture's bind groups: sampled nearest at full size, and,
/// for a [`SceneLayer::smooth`] texture, through its mipmaps:
/// anisotropically, and not.
#[derive(Clone)]
struct Bound {
    plain: wgpu::BindGroup,
    filtered: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
}

impl Bound {
    /// The bind group `camera`'s filtering samples it with.
    fn for_camera(&self, camera: &SceneCamera) -> &wgpu::BindGroup {
        match &self.filtered {
            Some((sharp, _)) if camera.filtered && camera.anisotropic => sharp,
            Some((_, smooth)) if camera.filtered => smooth,
            _ => &self.plain,
        }
    }
}

/// The pipeline cache for `adapter`'s driver, with what an earlier run kept
/// of it, if the device can keep one (`gpu`'s device asks for the feature).
fn pipeline_cache(
    device: &wgpu::Device,
    adapter: &wgpu::Adapter,
) -> Option<(wgpu::PipelineCache, std::path::PathBuf)> {
    if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return None;
    }
    let key = wgpu::util::pipeline_cache_key(&adapter.get_info())?;
    let path = crate::settings::Settings::dir()?.join(key);
    let data = std::fs::read(&path).ok();
    // SAFETY: the data is only ever what `get_data` gave for this driver
    // (its file is named for it), and `fallback` makes wgpu start an empty
    // cache should the driver reject it.
    let cache = unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("scene pipelines"),
            data: data.as_deref(),
            fallback: true,
        })
    };
    l3d_engine::log::info!(
        "pipeline cache {} ({} bytes kept)",
        path.display(),
        data.map_or(0, |d| d.len())
    );
    Some((cache, path))
}

/// Keeps `cache`'s compiled pipelines in `path` for the next run, replacing
/// the file whole (written aside, then renamed over it).
fn save_pipeline_cache(cache: &wgpu::PipelineCache, path: &std::path::Path) {
    let Some(data) = cache.get_data() else { return };
    let aside = path.with_extension("new");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&aside, &data).and_then(|()| std::fs::rename(&aside, path)) {
        l3d_engine::log::warn!("could not keep the pipeline cache: {e}");
    }
}

impl SceneGpu {
    pub fn new(device: &wgpu::Device, adapter: &wgpu::Adapter) -> Self {
        let depth_format = [
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureFormat::Depth24Plus,
        ]
        .into_iter()
        .find(|&f| {
            adapter
                .get_texture_format_features(f)
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        })
        .unwrap_or(wgpu::TextureFormat::Depth24Plus);
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
        let mipmapped = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene mipmapped"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let trilinear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene trilinear"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let extras_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene extras"),
            entries: &[
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // The waves worked out ahead (`WaveField`).
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let blank = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("scene blank"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let blank_view = blank.clone();
        let blank = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene blank"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blank),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
            ],
        });
        SceneGpu {
            cache: pipeline_cache(device, adapter),
            reflection: None,
            wave_field: None,
            blank,
            blank_view,
            extras_layout,
            field: None,
            layout,
            shader,
            sampler,
            linear,
            mipmapped,
            trilinear,
            uniforms,
            pipelines: HashMap::new(),
            low: None,
            version: None,
            layers: Vec::new(),
            sky: None,
            sprite_atlas: None,
            sprites: None,
            shadows: None,
            hdr: None,
            depth: None,
            depth_format,
            target_origin: (0, 0),
            target_size: (1, 1),
            textures: Vec::new(),
        }
    }

    /// The format of the depth buffer the scene draws with.
    pub fn depth_format(&self) -> wgpu::TextureFormat {
        self.depth_format
    }

    /// Compiles the shaders every level draws with into the usual target,
    /// ahead of the first frame (and Present, which an old Vulkan driver
    /// fails to compile, before the window is attached: see `gpu`); the
    /// rest wait until they are drawn with.
    pub fn compile(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        self.pipelines(
            device,
            format,
            &[Which::Geometry, Which::Sky, Which::Stretch, Which::Present],
            Features::default(),
        );
    }

    /// Makes pipeline `which` drawing into `format`.
    fn make(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        which: Which,
        features: Features,
    ) -> wgpu::RenderPipeline {
        // The level's features, as the scene shader's override constants.
        let constants = features.constants();
        let specialised = wgpu::PipelineCompilationOptions {
            constants: &constants,
            ..Default::default()
        };
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&self.layout)],
            ..Default::default()
        });
        // Presenting the glowing scene also samples the bloom chain's top
        // level, bound second.
        let pl_two = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene two pictures"),
            bind_group_layouts: &[Some(&self.layout), Some(&self.layout)],
            ..Default::default()
        });
        // Drawing the scene, the reflection and the signal field are bound
        // second.
        let pl_scene = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene with extras"),
            bind_group_layouts: &[Some(&self.layout), Some(&self.extras_layout)],
            ..Default::default()
        });
        let geometry = |label, cull| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pl_scene),
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
                compilation_options: specialised.clone(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(cull), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: self.depth_format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: self.cache.as_ref().map(|(c, _)| c),
        })
        };
        // A full-screen triangle over the scene, ignoring its depth:
        // replacing what is there, or blended over it by alpha.
        let over_scene = |label, fragment, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl_scene),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_sky"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::COLOR,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: self.depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: self.cache.as_ref().map(|(c, _)| c),
            })
        };
        let dust_sheets = || {
            over_scene(
                "scene dust sheets",
                "fs_dust_sheets",
                Some(wgpu::BlendState::ALPHA_BLENDING),
            )
        };
        // Particles in the world (snowflakes, sand grains): a quad each,
        // hidden by what is nearer.
        let particles = |label, vs, fs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl_scene),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(vs),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::COLOR,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: self.depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: self.cache.as_ref().map(|(c, _)| c),
            })
        };
        let snow = || particles("scene snow", "vs_snow", "fs_snow");
        let sand = || particles("scene sand", "vs_sand", "fs_sand");
        let sky = || {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene sky"),
                layout: Some(&pl_scene),
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
                // At the far plane (depth 0, the clear), so drawn only where
                // nothing is in front.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: self.depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: self.cache.as_ref().map(|(c, _)| c),
            })
        };
        // A full-screen triangle with no depth, optionally added onto what
        // is there.
        let full_screen = |label, vertex, fragment, add: bool, layout| {
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
                layout: Some(layout),
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
                cache: self.cache.as_ref().map(|(c, _)| c),
            })
        };
        let shadow = || {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene shadows"),
            layout: Some(&pl_scene),
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
                entry_point: Some("fs_shadow"),
                // What is there, times the shadow's colour.
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::Src,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: self.depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: self.cache.as_ref().map(|(c, _)| c),
        })
        };
        match which {
            Which::DustSheets => dust_sheets(),
            Which::Sand => sand(),
            Which::Snow => snow(),
            Which::WaveField => device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene wave field"),
                layout: Some(&pl_scene),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_sky"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_wave_field"),
                    targets: &[Some(WAVE_FIELD_FORMAT.into())],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: self.cache.as_ref().map(|(c, _)| c),
            }),
            Which::Shadow => shadow(),
            Which::Geometry => geometry("scene geometry", wgpu::Face::Back),
            // Mirrored, the faces turn over.
            Which::Mirrored => geometry("scene mirrored", wgpu::Face::Front),
            Which::Sky => sky(),
            Which::Stretch => full_screen("scene stretch", "vs_sky", "fs_stretch", false, &pl),
            Which::Present => full_screen("scene present", "vs_sky", "fs_present", false, &pl),
            Which::PresentGlow => full_screen(
                "scene present glow",
                "vs_sky",
                "fs_present_glow",
                false,
                &pl_two,
            ),
            Which::BloomFirst => {
                full_screen("bloom first", "vs_post", "fs_bloom_first", false, &pl)
            }
            Which::BloomDown => full_screen("bloom down", "vs_post", "fs_bloom_down", false, &pl),
            Which::BloomUp => full_screen("bloom up", "vs_post", "fs_bloom_up", true, &pl),
        }
    }

    /// Makes the pipelines `need` drawing into `format` that aren't made
    /// yet: each the first time it is drawn with, as compiling them is slow
    /// on some GPUs (seconds on a Mali-G31 for them all), and many are only
    /// for Enhanced or Ultra, or a few levels.
    /// Geometry pipelines are made for the level's `features` (others
    /// ignore them).
    fn pipelines(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        need: &[Which],
        features: Features,
    ) {
        let mut made = false;
        for &w in need {
            let key = (format, w, w.specialised(features));
            if !self.pipelines.contains_key(&key) {
                let p = self.make(device, format, w, key.2);
                self.pipelines.insert(key, p);
                made = true;
            }
        }
        if made && let Some((cache, path)) = &self.cache {
            save_pipeline_cache(cache, path);
        }
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
            for halving in 1..=BLOOM_LEVELS {
                if s.0.min(s.1) / 2 < BLOOM_MIN_SIZE {
                    break;
                }
                s = (s.0 / 2, s.1 / 2);
                if s.1 > BLOOM_TOP_ROWS && halving < BLOOM_LEVELS {
                    continue;
                }
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

    /// The picture of the level mirrored in the sea, of `size` in `format`,
    /// made anew when either changes.
    fn reflection_target(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> (wgpu::TextureView, wgpu::BindGroup) {
        if self
            .reflection
            .as_ref()
            .is_none_or(|(s, f, ..)| *s != size || *f != format)
        {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene reflection"),
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
            let bind = self.bind_view(device, &view, &self.linear);
            self.reflection = Some((size, format, view, bind));
        }
        let (_, _, view, bind) = self.reflection.as_ref().unwrap();
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
                    format: self.depth_format,
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
        smooth: bool,
    ) -> Bound {
        let (levels, texels) = if smooth {
            mip_chain(img)
        } else {
            (1, img.texels.to_vec())
        };
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("scene texture"),
                size: wgpu::Extent3d {
                    width: img.width,
                    height: img.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &texels,
        );
        let group = |view: &wgpu::TextureView, sampler: &wgpu::Sampler| {
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
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        let full_size = texture.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            ..Default::default()
        });
        Bound {
            plain: group(&full_size, &self.sampler),
            filtered: smooth.then(|| {
                let view = texture.create_view(&Default::default());
                (group(&view, &self.mipmapped), group(&view, &self.trilinear))
            }),
        }
    }
}

/// Uploads the uniforms, this frame's sprites and, when it changed, the
/// content.
#[allow(clippy::too_many_arguments)] // the frame's inputs, each its own
fn prepare(
    gpu: &mut SceneGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    content: &SceneContent,
    sprites: &SceneSprites,
    camera: &SceneCamera,
    headroom: f32,
    signals: Option<f32>,
    marks: Option<f32>,
) {
    let (w, h) = gpu.target_size;
    // The original's 3D view is 400 rows tall (its 640×400 mode); its sky
    // texels are 2 of those pixels wide (measured with a marked sky,
    // 2026-10-25: stripes 64 texels apart 128 pixels apart).
    let scale = h as f32 / 400.0;
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
    let mirror = content.data.as_ref().and_then(|d| d.mirror);
    uniforms.extend([
        if glow { GLOW_GAIN } else { 0.0 },
        headroom,
        if mirror.is_some() { 1.0 } else { 0.0 },
        if camera.crt { 1.0 } else { 0.0 },
    ]);
    // The camera's position: the point the projection takes to infinity
    // in every direction.
    let eye = camera.view_proj.inverse() * Vec4::new(0.0, 0.0, 1.0, 0.0);
    let eye = eye.truncate() / eye.w;
    uniforms.extend([eye.x, eye.y, eye.z, mirror.unwrap_or(0.0)]);
    let storm = content.data.as_ref().is_some_and(|d| d.storm);
    // The storm, and how much of it reaches the camera: none under cover.
    let (storm, around) = if storm {
        (1.0, 1.0 - camera.shelter)
    } else {
        (0.0, 0.0)
    };
    // Heat shimmer goes with the storm, over the same deserts; it is kept
    // in the alpha of the glowing scene, so only while that is drawn.
    let heat = if glow && camera.bloom { storm } else { 0.0 };
    // A night sky shows our stars and clouds only in our modes.
    let sky = match content.data.as_ref() {
        Some(d) if camera.ours && d.starry => 1.0,
        _ => 0.0,
    };
    uniforms.extend([storm, around, heat, sky]);
    let glossy = content
        .data
        .as_ref()
        .filter(|d| d.mirror.is_some())
        .map_or(0.0, |d| d.glossy);
    // The signal field's texels per unit, and whether it is bound.
    let (per_unit, field) = signals.map_or((0.0, 0.0), |p| (p, 1.0));
    // 1: what faces up at the mirror shines; 2: only slippery tops there.
    let slippery = content.data.as_ref().is_some_and(|d| d.glossy_slippery);
    let shines = match (glossy > 0.0, slippery) {
        (false, _) => 0.0,
        (true, false) => 1.0,
        (true, true) => 2.0,
    };
    uniforms.extend([shines, per_unit, field, glossy]);
    // Snow, and how much of it falls around the camera: none under cover.
    let snow = content.data.as_ref().is_some_and(|d| d.snow);
    let snow = if snow { 1.0 } else { 0.0 };
    // The marks in snow's texels per unit, bound as the field, or 0.
    let windows = content.data.as_ref().is_some_and(|d| d.windows);
    uniforms.extend([
        snow,
        snow * (1.0 - camera.shelter),
        marks.unwrap_or(0.0),
        if windows { 1.0 } else { 0.0 },
    ]);
    // The waves worked out ahead for the area around the level
    // (`WaveField`): its corner, the units a texel spans and its size in
    // texels; none, zero.
    uniforms.extend(match content.data.as_ref().and_then(|d| d.wave_area) {
        Some([x, z, extent]) => [x, z, extent / WAVE_FIELD as f32, WAVE_FIELD as f32],
        None => [0.0; 4],
    });
    // From clip space back to the world, for rays through the screen.
    uniforms.extend(camera.view_proj.inverse().to_cols_array());
    // The cells the sand grains are kept out of the lee of.
    let solid = content.data.as_ref().and_then(|d| d.solid.as_deref());
    uniforms.extend(
        (0..SOLID_WORDS)
            .map(|i| f32::from_bits(solid.and_then(|s| s.get(i)).copied().unwrap_or(0))),
    );
    queue.write_buffer(&gpu.uniforms, 0, &f32_bytes(&uniforms));
    upload_dynamic(
        device,
        queue,
        &mut gpu.sprites,
        &sprites.vertices,
        &sprites.indices,
    );
    upload_dynamic(
        device,
        queue,
        &mut gpu.shadows,
        &sprites.shadow_vertices,
        &sprites.shadow_indices,
    );
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
    let bind = |gpu: &mut SceneGpu, img: &RgbaImage, smooth: bool| {
        let same =
            |(t, s, _): &&(Arc<[u8]>, bool, Bound)| Arc::ptr_eq(t, &img.texels) && *s == smooth;
        if let Some((.., b)) = gpu.textures.iter().find(same) {
            return b.clone();
        }
        let b = match old.iter().find(same) {
            Some((.., b)) => b.clone(),
            None => gpu.bind_texture(device, queue, img, smooth),
        };
        gpu.textures.push((img.texels.clone(), smooth, b.clone()));
        b
    };
    let mut layers = Vec::new();
    for l in data.layers.iter().filter(|l| !l.indices.is_empty()) {
        let bind_group = bind(gpu, &l.texture, l.smooth);
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
            screen: l.screen,
            kind: l.kind,
        });
    }
    gpu.layers = layers;
    gpu.sky = data.sky.as_ref().map(|s| bind(gpu, s, true));
    gpu.sprite_atlas = data
        .sprite_atlas
        .as_ref()
        .map(|s| bind(gpu, s, false).plain);
}

/// Copies this frame's geometry into the GPU buffers in `slot`, growing
/// them when they are too small.
fn upload_dynamic(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    vertices: &[f32],
    indices: &[u32],
) {
    let count = indices.len() as u32;
    if count == 0 {
        if let Some(s) = slot {
            s.2 = 0;
        }
        return;
    }
    let vbytes = f32_bytes(vertices);
    let ibytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let fits = slot.as_ref().is_some_and(|(v, i, _)| {
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
        *slot = Some((
            alloc(vbytes.len(), wgpu::BufferUsages::VERTEX),
            alloc(ibytes.len(), wgpu::BufferUsages::INDEX),
            0,
        ));
    }
    let (v, i, n) = slot.as_mut().unwrap();
    queue.write_buffer(v, 0, &vbytes);
    queue.write_buffer(i, 0, &ibytes);
    *n = count;
}

impl SceneGpu {
    /// Draws the scene into `viewport` (x, y, width, height) of `target`, a
    /// frame of `full` size in `input.format`.
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
        let (mut signals, mut marks) = (None, None);
        if let Some((field, dirty)) = input.signals {
            let id = (Field::Signals, field.generation);
            self.upload_field(device, &gpu.queue, id, field.size, dirty, |i| {
                field.arrivals[i].to_le_bytes()
            });
            signals = Some(field.per_unit());
        } else if let Some((field, dirty)) = input.marks {
            let id = (Field::Marks, field.generation);
            self.upload_field(device, &gpu.queue, id, field.size, dirty, |i| {
                field.texels[i * 4..i * 4 + 4].try_into().unwrap()
            });
            marks = Some(crate::marks::PER_UNIT);
        } else {
            self.field = None;
        }
        prepare(
            self,
            device,
            &gpu.queue,
            input.content,
            input.sprites,
            input.camera,
            input.headroom,
            signals,
            marks,
        );
        draw(self, device, encoder, target, full, viewport, input);
    }

    /// Brings the GPU's copy of the level's field `id` (`(w, h)` texels,
    /// texel `i` row by row being `texel(i)`) up to date: made and filled
    /// whole for a new field, else its `dirty` part rewritten.
    fn upload_field(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: (Field, u64),
        (w, h): (u32, u32),
        dirty: Option<[u32; 4]>,
        texel: impl Fn(usize) -> [u8; 4],
    ) {
        let fresh = self.field.as_ref().is_none_or(|s| (s.0, s.1) != id);
        if fresh {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("level field"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.field = Some((id.0, id.1, texture, view));
        }
        let rect = if fresh { Some([0, 0, w, h]) } else { dirty };
        let (Some([x0, y0, x1, y1]), Some((_, _, texture, _))) = (rect, &self.field) else {
            return;
        };
        let bytes: Vec<u8> = (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (y * w + x) as usize))
            .flat_map(texel)
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: x0, y: y0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((x1 - x0) * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: x1 - x0,
                height: y1 - y0,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn draw(
    gpu: &mut SceneGpu,
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    full: UVec2,
    viewport: UVec4,
    input: &SceneInput,
) {
    let (camera, profile) = (input.camera, input.profile);
    let size = gpu.target_size;
    let format = input.format;
    let device = device.clone();
    // Rendered small and stretched over the screen, or at the screen's size:
    // offscreen in floating point while glowing, else straight onto it.
    let low = renders_small(camera.pixels, size).then(|| {
        let s = (camera.pixels.x, camera.pixels.y);
        (s, gpu.low_target(&device, s, format))
    });
    let hdr =
        (camera.glow && camera.bloom && low.is_none()).then(|| gpu.hdr_targets(&device, size));
    let scene_format = if hdr.is_some() { HDR_FORMAT } else { format };
    let depth_size = match (&low, &hdr) {
        (Some(l), _) => l.0,
        (_, Some(h)) => h.size,
        _ => (full.x, full.y),
    };
    let depth = gpu.depth_view(&device, depth_size);
    // The reflection is drawn at the same size, in the same place.
    let mirror = input
        .content
        .data
        .as_ref()
        .is_some_and(|d| d.mirror.is_some());
    let reflection = mirror.then(|| gpu.reflection_target(&device, depth_size, scene_format));
    let storm = input.content.data.as_ref().is_some_and(|d| d.storm);
    let snow = input.content.data.as_ref().is_some_and(|d| d.snow);
    // The pipelines this frame draws with, made the first time (`pipelines`).
    let mut need = Vec::new();
    for (on, which) in [
        (gpu.sky.is_some(), Which::Sky),
        (gpu.shadows.as_ref().is_some_and(|s| s.2 > 0), Which::Shadow),
        (snow, Which::Snow),
        (storm, Which::Sand),
        (storm, Which::DustSheets),
    ] {
        if on {
            need.push(which);
        }
    }
    // What the level shows, which its geometry's shader is compiled for.
    let features = input
        .content
        .data
        .as_ref()
        .map_or(Features::default(), |d| Features {
            windows: d.windows,
            screens: camera.crt && d.layers.iter().any(|l| l.screen),
            sea: d.waves,
            circuit: input.signals.is_some(),
            snow: d.snow || d.snow_tiles.is_some() || input.marks.is_some(),
            gloss: d.glossy > 0.0,
            storm: d.storm,
            mirror: d.mirror.is_some(),
        });
    gpu.pipelines(&device, scene_format, &need, features);
    // The geometry's pipelines, one for each kind of layer the level has
    // (and the sprites).
    let kinds: Vec<LayerKind> = gpu
        .layers
        .iter()
        .map(|l| l.kind)
        .chain([LayerKind::Objects])
        .collect();
    for kind in kinds {
        let geometry: &[Which] = if mirror {
            &[Which::Geometry, Which::Mirrored]
        } else {
            &[Which::Geometry]
        };
        gpu.pipelines(&device, scene_format, geometry, features.for_layer(kind));
    }
    let bloom = hdr.as_ref().is_some_and(|h| !h.levels.is_empty());
    if bloom {
        gpu.pipelines(
            &device,
            HDR_FORMAT,
            &[Which::BloomFirst, Which::BloomDown, Which::BloomUp],
            features,
        );
    }
    let shown = match (&low, &hdr) {
        (Some(_), _) => Some(Which::Stretch),
        (_, Some(_)) if bloom => Some(Which::PresentGlow),
        (_, Some(_)) => Some(Which::Present),
        _ => None,
    };
    if let Some(w) = shown {
        gpu.pipelines(&device, format, &[w], features);
    }
    // The waves worked out ahead, where the sea has them.
    let wave_area = input
        .content
        .data
        .as_ref()
        .and_then(|d| d.wave_area)
        .filter(|_| features.sea);
    if wave_area.is_some() {
        gpu.pipelines(&device, WAVE_FIELD_FORMAT, &[Which::WaveField], features);
        if gpu.wave_field.is_none() {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene wave field"),
                    size: wgpu::Extent3d {
                        width: WAVE_FIELD,
                        height: WAVE_FIELD,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: WAVE_FIELD_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            gpu.wave_field = Some(view);
        }
    }
    // Only bare skies are sampled smoothly: stars, planets and clouds gain;
    // scenery along a horizon (trees, castles, hills) only blurs.
    let bare = input.content.data.as_ref().is_some_and(|d| d.sky_bare);
    // The last frame, for layers showing the screen.
    let screen = input
        .screen
        .filter(|_| gpu.layers.iter().any(|l| l.screen))
        .map(|view| gpu.bind_view(&device, view, &gpu.linear));
    let gpu = &*gpu;
    let waves = wave_area.and(gpu.wave_field.as_ref());
    // The scene's second bind group: the reflection when it is drawn, the
    // level's field (signals or marks), either blank.
    let extras = |reflection: Option<&wgpu::TextureView>, waves: Option<&wgpu::TextureView>| {
        let field = gpu.field.as_ref().map(|(_, _, _, view)| view);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene extras"),
            layout: &gpu.extras_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        reflection.unwrap_or(&gpu.blank_view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gpu.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(field.unwrap_or(&gpu.blank_view)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(waves.unwrap_or(&gpu.blank_view)),
                },
            ],
        })
    };
    if gpu.layers.is_empty() && gpu.sky.is_none() && gpu.sprites.is_none() {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(input.clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        if let Some(overlay) = input.overlay {
            overlay(&mut pass, false);
        }
        drop(pass);
        profile.stamp(encoder, Stamp::Geometry);
        profile.stamp(encoder, Stamp::Bloom);
        return;
    }
    let pipeline = |f, w: Which| {
        gpu.pipelines
            .get(&(f, w, w.specialised(features)))
            .expect("made above")
    };
    let scene_p = |w| pipeline(scene_format, w);
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
    // The first pass onto the target clears it; the later ones draw over.
    let cleared = std::cell::Cell::new(false);
    let onto_target = || wgpu::RenderPassColorAttachment {
        view: target,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: if cleared.replace(true) {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(input.clear)
            },
            store: wgpu::StoreOp::Store,
        },
    };
    // The sky, the geometry and the sprites, either mirrored in the sea
    // (their second instance, without shadows) or as they are, with
    // `second` bound as the reflection.
    // The geometry's pipeline for a kind of layer.
    let geometry_for = |mirrored: bool, kind: LayerKind| {
        let w = if mirrored {
            Which::Mirrored
        } else {
            Which::Geometry
        };
        gpu.pipelines
            .get(&(scene_format, w, features.for_layer(kind)))
            .expect("made above")
    };
    let scene = |pass: &mut wgpu::RenderPass, mirrored: bool, second: &wgpu::BindGroup| {
        let instance = if mirrored { 1..2 } else { 0..1 };
        let geometry = geometry_for(mirrored, LayerKind::Objects);
        pass.set_bind_group(1, second, &[]);
        for layer in &gpu.layers {
            pass.set_pipeline(geometry_for(mirrored, layer.kind));
            let own = layer.bind_group.for_camera(camera);
            let bind = match &screen {
                Some(s) if layer.screen => s,
                _ => own,
            };
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, layer.vertices.slice(..));
            pass.set_index_buffer(layer.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..layer.index_count, 0, instance.clone());
        }
        // Shadows on what is drawn so far, before the sprites that cast them.
        if let (false, Some(atlas), Some((v, i, n))) = (mirrored, &gpu.sprite_atlas, &gpu.shadows)
            && *n > 0
        {
            pass.set_pipeline(scene_p(Which::Shadow));
            pass.set_bind_group(0, atlas, &[]);
            pass.set_vertex_buffer(0, v.slice(..));
            pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*n, 0, 0..1);
            pass.set_pipeline(geometry);
        }
        if let (Some(atlas), Some((v, i, n))) = (&gpu.sprite_atlas, &gpu.sprites)
            && *n > 0
        {
            pass.set_pipeline(geometry);
            pass.set_bind_group(0, atlas, &[]);
            pass.set_vertex_buffer(0, v.slice(..));
            pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*n, 0, instance.clone());
        }
        // The sky last, where nothing else is (its depth test passes only
        // where nothing was drawn): drawn first, it was drawn under the sea
        // and the land too, each of their pixels twice.
        if let Some(sky) = &gpu.sky {
            pass.set_pipeline(scene_p(Which::Sky));
            let sky = if bare {
                sky.for_camera(camera)
            } else {
                &sky.plain
            };
            pass.set_bind_group(0, sky, &[]);
            pass.draw(0..3, instance.clone());
        }
        // Snowflakes, behind what is nearer.
        if snow && !mirrored {
            pass.set_pipeline(scene_p(Which::Snow));
            pass.set_bind_group(0, &gpu.blank, &[]);
            pass.draw(0..6, 0..SNOW_FLAKES);
        }
        // Sand grains blown along the wind, behind what is nearer; then
        // sheets of dust sweeping across the view, over everything.
        if storm && !mirrored {
            pass.set_pipeline(scene_p(Which::Sand));
            pass.set_bind_group(0, &gpu.blank, &[]);
            pass.draw(0..6, 0..SAND_GRAINS);
            pass.set_pipeline(scene_p(Which::DustSheets));
            pass.draw(0..3, 0..1);
        }
    };
    let depth_attachment = || {
        Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        })
    };
    // This frame's waves, ahead of everything reading them.
    if let Some(view) = waves {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene wave field"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline(WAVE_FIELD_FORMAT, Which::WaveField));
        pass.set_bind_group(0, &gpu.blank, &[]);
        pass.set_bind_group(1, &extras(None, None), &[]);
        pass.draw(0..3, 0..1);
    }
    if let Some((view, _)) = &reflection {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene reflection"),
            color_attachments: &[Some(offscreen(view))],
            depth_stencil_attachment: depth_attachment(),
            ..Default::default()
        });
        if low.is_none() && hdr.is_none() {
            set_viewport(&mut pass, viewport);
        }
        scene(&mut pass, true, &extras(None, waves));
    }
    let color = match (&low, &hdr) {
        (Some((_, (view, _))), _) => offscreen(view),
        (_, Some(h)) => offscreen(&h.view),
        _ => onto_target(),
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(color)],
        depth_stencil_attachment: depth_attachment(),
        ..Default::default()
    });
    if low.is_none() && hdr.is_none() {
        set_viewport(&mut pass, viewport);
    }
    scene(
        &mut pass,
        false,
        &extras(reflection.as_ref().map(|(view, _)| view), waves),
    );
    // Drawn straight onto the target: the last pass there.
    if low.is_none()
        && hdr.is_none()
        && let Some(overlay) = input.overlay
    {
        overlay(&mut pass, true);
    }
    drop(pass);
    profile.stamp(encoder, Stamp::Geometry);
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
                0 => step(
                    view,
                    black,
                    pipeline(HDR_FORMAT, Which::BloomFirst),
                    &h.smooth,
                ),
                _ => step(
                    view,
                    black,
                    pipeline(HDR_FORMAT, Which::BloomDown),
                    &h.levels[k - 1].1,
                ),
            }
        }
        for k in (1..h.levels.len()).rev() {
            step(
                &h.levels[k - 1].0,
                wgpu::LoadOp::Load,
                pipeline(HDR_FORMAT, Which::BloomUp),
                &h.levels[k].1,
            );
        }
    }
    profile.stamp(encoder, Stamp::Bloom);
    let shown = match (&low, &hdr) {
        (Some((_, (_, bind))), _) => Some((bind, pipeline(format, Which::Stretch), None)),
        (_, Some(h)) => Some(match h.levels.first() {
            Some((_, first)) => (&h.sharp, pipeline(format, Which::PresentGlow), Some(first)),
            None => (&h.sharp, pipeline(format, Which::Present), None),
        }),
        _ => None,
    };
    if let Some((bind, pipeline, bloom)) = shown {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene stretch"),
            color_attachments: &[Some(onto_target())],
            ..Default::default()
        });
        set_viewport(&mut pass, viewport);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        if let Some(bloom) = bloom {
            pass.set_bind_group(1, bloom, &[]);
        }
        pass.draw(0..3, 0..1);
        if let Some(overlay) = input.overlay {
            overlay(&mut pass, false);
        }
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

/// `img` and its mipmaps down to one texel across, each a 2×2 box average
/// in linear light of the one before, all in one buffer: the level count
/// and the texels. A strip of square frames stacked down the texture keeps
/// its frames apart, as no box straddles two frames.
fn mip_chain(img: &RgbaImage) -> (u32, Vec<u8>) {
    let to_linear = |c: u8| (c as f32 / 255.0).powf(2.2);
    let to_srgb = |c: f32| (c.powf(1.0 / 2.2) * 255.0).round() as u8;
    let mut out = img.texels.to_vec();
    let (mut w, mut h) = (img.width as usize, img.height as usize);
    let mut level = img.texels.to_vec();
    let mut levels = 1;
    while w > 1 && w.is_multiple_of(2) && h.is_multiple_of(2) {
        let (nw, nh) = (w / 2, h / 2);
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| level[((2 * y + dy) * w + 2 * x + dx) * 4 + c];
                    let texels = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
                    next[(y * nw + x) * 4 + c] = if c == 3 {
                        (texels.iter().map(|&a| a as u32).sum::<u32>() / 4) as u8
                    } else {
                        to_srgb(texels.iter().map(|&t| to_linear(t)).sum::<f32>() / 4.0)
                    };
                }
            }
        }
        out.extend_from_slice(&next);
        (w, h, level) = (nw, nh, next);
        levels += 1;
    }
    (levels, out)
}

#[cfg(test)]
mod tests {
    use super::Features;
    use wgpu::naga;

    fn module() -> naga::Module {
        let source = include_str!("scene.wgsl");
        naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)))
    }

    /// The scene's shaders parse and validate, as a GPU would take them.
    #[test]
    fn scene_shader_is_valid() {
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module())
        .unwrap_or_else(|e| panic!("{e:?}"));
    }

    /// Every feature constant the pipelines set is one of the shader's
    /// overrides (wgpu refuses a pipeline naming one that isn't).
    #[test]
    fn feature_constants_are_overrides() {
        let module = module();
        let names: Vec<_> = module
            .overrides
            .iter()
            .filter_map(|(_, o)| o.name.clone())
            .collect();
        for (name, _) in Features::default().constants() {
            assert!(names.iter().any(|n| n == name), "no override {name}");
        }
    }
}
