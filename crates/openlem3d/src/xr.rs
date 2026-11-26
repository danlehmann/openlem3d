//! VR on standalone headsets (Meta Quest), through OpenXR: the `xr` build.
//!
//! OpenXR makes the Vulkan instance and device (`XR_KHR_vulkan_enable2`), and
//! wgpu is built on them through wgpu-hal, so the game's own renderer draws
//! straight into the headset's images. Each eye sees the level from where the
//! head is: the player's camera (`ViewCamera`) is where the head was when the
//! session began, facing the camera's way, and the head's tracked moves and
//! turns go on from there. The camera controls still move that anchor, which
//! is how the player gets about.
//!
//! The UI (menus, briefings, the skill panel) is drawn into a panel of its
//! own that floats below and ahead, and a controller's ray stands for the
//! mouse on it: where it meets the panel is the pointer, the trigger the
//! left button.

use std::ffi::c_char;
use std::time::Instant;

use ash::vk::{self, Handle};
use l3d_engine::prelude::*;
use openxr as xr;
use wgpu::hal;

use crate::World;
use crate::gpu::{Gpu, TARGET_FORMAT};
use crate::scene_render::{SceneCamera, SceneGpu, SceneInput};
use crate::ui::UiGpu;

const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;
/// The headset's images: sRGB RGBA8, the format the game draws in.
const SWAPCHAIN_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
/// Metres per grid unit: the level life-size around the player (ours).
const METRES_PER_UNIT: f32 = 1.0;
/// How fast the right stick raises and lowers the player (grid units per
/// second at full tilt).
const RISE_SPEED: f32 = 3.0;
/// The skill palette on the left hand while playing (`panel::palette_pixels`,
/// a tall strip): its width in metres, its centre from the hand's grip
/// (metres, in the grip's axes), and how far it leans back from upright
/// towards the player (radians).
const PALETTE_WIDTH: f32 = 0.12;
const PALETTE_OFFSET: Vec3 = Vec3::new(0.06, 0.24, -0.06);
const PALETTE_TILT: f32 = -0.6;
/// The level's menu as a popup: its width, how far ahead of the head and
/// how far below its height it opens (metres), and how far it leans back
/// (radians).
const POPUP_WIDTH: f32 = 0.7;
const POPUP_AHEAD: f32 = 0.9;
const POPUP_BELOW: f32 = 0.12;
const POPUP_TILT: f32 = 0.12;
/// The time control's bar (`time_dial`): on a panel of its own ahead at
/// eye height while a squeeze runs the time, this wide (metres).
const DIAL_WIDTH: f32 = 0.6;
const DIAL_AHEAD: f32 = 1.0;
/// How long the view stays dark when cutting to a camera view (seconds).
const JUMP_DARK: f32 = 0.15;
/// How long a glide to a camera view may take to begin after its key
/// (seconds).
const GLIDE_START: f32 = 0.1;
/// The near plane, in metres.
const NEAR: f32 = 0.05;
/// The UI panel: its width in metres, its centre relative to where the head
/// began (below and ahead), and how far it leans back, in radians.
const PANEL_WIDTH: f32 = 1.2;
const PANEL_CENTRE: Vec3 = Vec3::new(0.0, -0.45, -1.0);
const PANEL_TILT: f32 = 0.5;
/// Thumbstick travel below which it counts as centred.
const STICK_DEAD: f32 = 0.2;
/// The level list's scrolling with the right stick fully over, rows per
/// second.
const LIST_ROWS_PER_SEC: f32 = 8.0;
/// A snap turn with the right stick, in radians.
const SNAP_TURN: f32 = std::f32::consts::FRAC_PI_6;
/// Smooth turning (the Smooth Turn setting) with the stick fully over,
/// radians per second.
const SMOOTH_TURN_SPEED: f32 = 1.75;
/// The comfort vignette (the Vignette setting): clear inside this radius
/// of each eye's view (as a fraction of its half-height and half-width), at
/// its darkest from the edge out, and how long it takes to come and go
/// (seconds).
const VIGNETTE_INNER: f32 = 0.45;
const VIGNETTE_EDGE: f32 = 0.95;
const VIGNETTE_DARK: f32 = 0.9;
const VIGNETTE_FADE: f32 = 0.2;
/// The field of view of the game's camera along the ray (degrees): only
/// the middle, where the pointer is, matters.
const AIM_FOV_Y: f32 = 30.0;
/// The ray: drawn at most this long (metres), this wide, and the dot at its
/// end this big (radians across, as seen).
const BEAM_LONGEST: f32 = 30.0;
const BEAM_WIDTH: f32 = 0.004;
const DOT_SIZE: f32 = 0.012;
/// The ray's colour (linear, straight alpha) on the panel, into the level,
/// and on a lemming that a click would pick.
const BEAM_PANEL: Vec4 = Vec4::new(1.0, 1.0, 1.0, 0.8);
const BEAM_LEVEL: Vec4 = Vec4::new(0.55, 0.8, 1.0, 0.7);
const BEAM_LEMMING: Vec4 = Vec4::new(1.0, 0.8, 0.1, 0.95);
/// The overlay the ray is drawn into is this fraction of the eyes'
/// resolution.
const OVERLAY_SCALE: u32 = 2;

/// The controller's ray as drawn: from the hand to what it meets, in the
/// headset's space.
#[derive(Clone, Copy)]
struct Beam {
    start: Vec3,
    end: Vec3,
    colour: Vec4,
}

/// Where the level is around the player: the level's point where the head
/// began (`pos`), the heading there (`yaw`, as `ViewCamera::yaw`), and the
/// level's size (metres per grid unit).
#[derive(Clone, Copy, Debug)]
struct Place {
    pos: Vec3,
    yaw: f32,
    mpu: f32,
}

impl Place {
    /// At the game camera, life-size.
    fn at_view(view: &crate::ViewCamera) -> Place {
        Place {
            pos: view.pos,
            yaw: view.yaw,
            mpu: METRES_PER_UNIT,
        }
    }

    /// The level's coordinates of a point in the headset's space.
    fn world_from_space(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(1.0 / self.mpu),
            anchor_rotation(self.yaw),
            self.pos,
        )
    }
}

/// Where the UI's image is shown, in the headset's space: its centre, turn
/// and width; on the palette, pointing through its empty parts reaches the
/// level.
#[derive(Clone, Copy)]
struct PanelPose {
    centre: Vec3,
    rotation: Quat,
    width: f32,
    palette: bool,
    /// The level's menu as a popup: only its card.
    popup: bool,
}

/// The layer drawn over everything (the panel too) holding the ray.
struct Overlay {
    chain: Chain,
    pipeline: wgpu::RenderPipeline,
}

/// Whether VR is running, not yet tried, or unavailable (the game then draws
/// into its window as usual).
#[derive(Default)]
pub(crate) enum State {
    #[default]
    Untried,
    Off,
    On(Box<Xr>),
}

pub(crate) struct Xr {
    instance: xr::Instance,
    system: xr::SystemId,
    session: xr::Session<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<xr::Vulkan>,
    /// Where the head was when the session began, on the floor's level.
    space: xr::Space,
    blend: xr::EnvironmentBlendMode,
    events: xr::EventDataBuffer,
    running: bool,
    focused: bool,
    eyes: Option<Chain>,
    panel: Option<Chain>,
    controls: Controls,
    /// The display time of the last frame, for placing the controllers.
    time: Option<xr::Time>,
    /// Which hand holds the left button down, if any.
    clicking: Option<usize>,
    /// The hand pointing (0 left, 1 right).
    hand: usize,
    /// The game camera's view along the ray, while it points into the level.
    aim: Option<Mat4>,
    /// The ray to draw, in the headset's space.
    beam: Option<Beam>,
    overlay: Option<Overlay>,
    /// Whether the right stick has been back in the middle since its last
    /// snap turn.
    turn_ready: bool,
    /// How far the comfort vignette is in (0–1).
    vignette: f32,
    /// Where the head is in the level, in units, and its heading (the
    /// camera-yaw convention), as of this frame's input.
    seen_from: Option<(Vec3, f32)>,
    place: Place,
    /// The level the place was set for, whether the lemming view was on,
    /// and whether the level was being played (not previewed): any of them
    /// changing sets it afresh.
    placed_for: (Option<u32>, bool, bool),
    /// When a cut to a camera view began (real seconds), while the view is
    /// dark for it.
    jump: Option<f32>,
    /// When a glide to a camera view began (real seconds), while the place
    /// rides along with the game's camera (the VR Glide option).
    gliding: Option<f32>,
    /// A key pressed for the game last frame, let go this one.
    key: Option<KeyCode>,
    /// Where the frame's time goes, logged now and then.
    timing: Timing,
    /// Where the UI was shown last.
    panel_pose: PanelPose,
    /// Where the level's menu popped up, ahead of the head when it opened.
    popup_at: Option<PanelPose>,
    /// Where the time control's bar shows, ahead of the head when it began.
    dial_at: Option<PanelPose>,
    scene: SceneGpu,
    ui: Option<UiGpu>,
}

/// A swapchain and its images as wgpu textures, with a view per layer.
struct Chain {
    handle: xr::Swapchain<xr::Vulkan>,
    size: UVec2,
    images: Vec<(wgpu::Texture, Vec<wgpu::TextureView>)>,
}

struct Controls {
    set: xr::ActionSet,
    select: xr::Action<bool>,
    /// The middle-finger buttons, as far as squeezed: the left runs the
    /// level back, the right ahead, as a gamepad's triggers do.
    squeeze: xr::Action<f32>,
    stick: xr::Action<xr::Vector2f>,
    /// As on a gamepad: Y the next camera view, B pause, X ride along with
    /// the lemming under the ray (or leave its view); A clicks as the
    /// trigger does.
    next: xr::Action<bool>,
    pause: xr::Action<bool>,
    ride: xr::Action<bool>,
    /// The left controller's menu button: the game's menu (Escape: pause,
    /// restart or leave).
    menu: xr::Action<bool>,
    /// The left and right hands' aim and grip.
    aims: [xr::Space; 2],
    grips: [xr::Space; 2],
    hands: [xr::Path; 2],
}

/// Starts VR if this is a headset; `None` (and a note in the log why)
/// otherwise. Makes the GPU.
fn start(profile: bool) -> Option<(Box<Xr>, Gpu)> {
    match unsafe { try_start(profile) } {
        Ok(x) => Some(x),
        Err(e) => {
            warn!("no VR: {e}");
            None
        }
    }
}

type Error = Box<dyn std::error::Error>;

unsafe fn try_start(profile: bool) -> Result<(Box<Xr>, Gpu), Error> {
    let app = l3d_engine::android::app();
    let platform = unsafe { xr::AndroidPlatformInfo::new(app.vm_as_ptr(), app.activity_as_ptr()) };
    let entry = unsafe { xr::Entry::load(&platform) }?;
    let available = entry.enumerate_extensions()?;
    if !available.khr_vulkan_enable2 {
        return Err("the OpenXR runtime has no Vulkan".into());
    }
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    extensions.khr_android_create_instance = true;
    let instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "openlem3d",
            application_version: 1,
            engine_name: "openlem3d",
            engine_version: 1,
            api_version: xr::Version::new(1, 0, 0),
        },
        &extensions,
        &[],
        &platform,
    )?;
    let props = instance.properties()?;
    info!(
        "OpenXR runtime {} {}",
        props.runtime_name, props.runtime_version
    );
    let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let blend = instance.enumerate_environment_blend_modes(system, VIEW_TYPE)?[0];
    let reqs = instance.graphics_requirements::<xr::Vulkan>(system)?;
    let (gpu, vk) = unsafe { vulkan(&instance, system, reqs, profile) }?;

    let (session, waiter, stream) = unsafe {
        instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk.instance.as_raw() as _,
                physical_device: vk.physical.as_raw() as _,
                device: vk.device.as_raw() as _,
                queue_family_index: vk.family,
                queue_index: 0,
            },
        )
    }?;
    let space =
        session.create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)?;
    let controls = controls(&instance, &session)?;
    let mut scene = SceneGpu::new(&gpu.device, &gpu.adapter);
    scene.compile(&gpu.device, TARGET_FORMAT);
    let xr = Xr {
        instance,
        system,
        session,
        waiter,
        stream,
        space,
        blend,
        events: xr::EventDataBuffer::new(),
        running: false,
        focused: false,
        eyes: None,
        panel: None,
        controls,
        time: None,
        clicking: None,
        hand: 1,
        aim: None,
        beam: None,
        overlay: None,
        turn_ready: true,
        vignette: 0.0,
        seen_from: None,
        place: Place {
            pos: Vec3::ZERO,
            yaw: 0.0,
            mpu: METRES_PER_UNIT,
        },
        placed_for: (None, false, false),
        jump: None,
        gliding: None,
        key: None,
        timing: Timing::default(),
        panel_pose: front_panel(),
        popup_at: None,
        dial_at: None,
        scene,
        ui: None,
    };
    Ok((Box::new(xr), gpu))
}

/// The raw Vulkan objects the session is made for.
struct RawVulkan {
    instance: vk::Instance,
    physical: vk::PhysicalDevice,
    device: vk::Device,
    family: u32,
}

/// Has OpenXR make the Vulkan instance and device, with the extensions and
/// features wgpu wants, and builds wgpu on them.
unsafe fn vulkan(
    instance: &xr::Instance,
    system: xr::SystemId,
    reqs: xr::vulkan::Requirements,
    profile: bool,
) -> Result<(Gpu, RawVulkan), Error> {
    type Api = hal::api::Vulkan;
    let entry = unsafe { ash::Entry::load() }?;
    let loader_version =
        unsafe { entry.try_enumerate_instance_version() }?.unwrap_or(vk::API_VERSION_1_0);
    let max = vk::make_api_version(
        0,
        reqs.max_api_version_supported.major() as u32,
        reqs.max_api_version_supported.minor() as u32,
        0,
    );
    let api_version = loader_version.min(max).min(vk::API_VERSION_1_3);
    if api_version < vk::API_VERSION_1_1 {
        return Err("Vulkan 1.1 is needed".into());
    }
    let flags = wgpu::InstanceFlags::from_build_config().with_env();
    let instance_extensions =
        <Api as hal::Api>::Instance::desired_extensions(&entry, api_version, flags)?;
    let names: Vec<*const c_char> = instance_extensions.iter().map(|e| e.as_ptr()).collect();
    let app_info = vk::ApplicationInfo::default()
        .application_name(c"openlem3d")
        .engine_name(c"openlem3d")
        .api_version(api_version);
    let create = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&names);
    let get_proc = entry.static_fn().get_instance_proc_addr;
    let raw_instance = unsafe {
        instance.create_vulkan_instance(
            system,
            std::mem::transmute::<
                vk::PFN_vkGetInstanceProcAddr,
                xr::sys::platform::VkGetInstanceProcAddr,
            >(get_proc),
            &create as *const _ as *const _,
        )
    }?
    .map_err(|r| format!("Vulkan instance: {:?}", vk::Result::from_raw(r)))?;
    let vk_instance = vk::Instance::from_raw(raw_instance as _);
    let ash_instance = unsafe { ash::Instance::load(entry.static_fn(), vk_instance) };
    let sdk = std::fs::read_to_string("/system/build.prop")
        .ok()
        .and_then(|p| {
            p.lines()
                .find_map(|l| l.strip_prefix("ro.build.version.sdk="))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or(32);
    let hal_instance = unsafe {
        <Api as hal::Api>::Instance::from_raw(
            entry.clone(),
            ash_instance.clone(),
            api_version,
            sdk,
            None,
            instance_extensions,
            flags,
            Default::default(),
            false,
            None,
        )
    }?;
    let physical = vk::PhysicalDevice::from_raw(unsafe {
        instance.vulkan_graphics_device(system, raw_instance)
    }? as _);
    let exposed = hal_instance
        .expose_adapter(physical)
        .ok_or("wgpu can't use the headset's GPU")?;
    let features = if profile {
        exposed.features & crate::profile::FEATURES
    } else {
        wgpu::Features::empty()
    };
    let limits = exposed.capabilities.limits.clone();
    let family = unsafe { ash_instance.get_physical_device_queue_family_properties(physical) }
        .iter()
        .position(|f| {
            f.queue_flags
                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
        })
        .ok_or("no graphics queue")? as u32;
    let device_extensions = exposed.adapter.required_device_extensions(features);
    let mut device_features = exposed
        .adapter
        .physical_device_features(&device_extensions, features);
    let names: Vec<*const c_char> = device_extensions.iter().map(|e| e.as_ptr()).collect();
    let priorities = [1.0];
    let queues = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&priorities)];
    let create = device_features.add_to_device_create(
        vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&names),
    );
    let raw_device = unsafe {
        instance.create_vulkan_device(
            system,
            std::mem::transmute::<
                vk::PFN_vkGetInstanceProcAddr,
                xr::sys::platform::VkGetInstanceProcAddr,
            >(get_proc),
            physical.as_raw() as _,
            &create as *const _ as *const _,
        )
    }?
    .map_err(|r| format!("Vulkan device: {:?}", vk::Result::from_raw(r)))?;
    let vk_device = vk::Device::from_raw(raw_device as _);
    let ash_device = unsafe { ash::Device::load(ash_instance.fp_v1_0(), vk_device) };
    let open = unsafe {
        exposed.adapter.device_from_raw(
            ash_device,
            None,
            &device_extensions,
            features,
            &limits,
            &wgpu::MemoryHints::default(),
            family,
            0,
        )
    }?;
    let wgpu_instance = unsafe { wgpu::Instance::from_hal::<Api>(hal_instance) };
    let adapter = unsafe { wgpu_instance.create_adapter_from_hal(exposed) };
    info!("{:?}", adapter.get_info());
    let (device, queue) = unsafe {
        adapter.create_device_from_hal(
            open,
            &wgpu::DeviceDescriptor {
                label: Some("openlem3d"),
                required_features: features,
                required_limits: limits,
                ..Default::default()
            },
        )
    }?;
    let gpu = Gpu::from_parts(wgpu_instance, adapter, device, queue);
    let raw = RawVulkan {
        instance: vk_instance,
        physical,
        device: vk_device,
        family,
    };
    Ok((gpu, raw))
}

/// The controllers: each hand's aim, grip, trigger, grip button and
/// thumbstick, and the face buttons.
fn controls(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> xr::Result<Controls> {
    let hands = [
        instance.string_to_path("/user/hand/left")?,
        instance.string_to_path("/user/hand/right")?,
    ];
    let set = instance.create_action_set("game", "Game", 0)?;
    let aim = set.create_action::<xr::Posef>("aim", "Aim", &hands)?;
    let grip_pose = set.create_action::<xr::Posef>("grip_pose", "Grip pose", &hands)?;
    let select = set.create_action::<bool>("select", "Select", &hands)?;
    let squeeze = set.create_action::<f32>("time", "Run time back or ahead", &hands)?;
    let stick = set.create_action::<xr::Vector2f>("stick", "Thumbstick", &hands)?;
    let next = set.create_action::<bool>("next_view", "Next camera view", &[])?;
    let pause = set.create_action::<bool>("pause", "Pause", &[])?;
    let ride = set.create_action::<bool>("ride", "Ride along", &[])?;
    let menu = set.create_action::<bool>("menu", "Menu", &[])?;
    let path = |p: &str| instance.string_to_path(p);
    let mut bindings = Vec::new();
    for side in ["left", "right"] {
        let input = |what: &str| path(&format!("/user/hand/{side}/input/{what}"));
        bindings.push(xr::Binding::new(&aim, input("aim/pose")?));
        bindings.push(xr::Binding::new(&grip_pose, input("grip/pose")?));
        bindings.push(xr::Binding::new(&select, input("trigger/value")?));
        bindings.push(xr::Binding::new(&squeeze, input("squeeze/value")?));
        bindings.push(xr::Binding::new(&stick, input("thumbstick")?));
    }
    // As a gamepad's: A gives the skill (as the trigger does), B pauses, X
    // rides along, Y the next camera view.
    bindings.push(xr::Binding::new(
        &select,
        path("/user/hand/right/input/a/click")?,
    ));
    bindings.push(xr::Binding::new(
        &pause,
        path("/user/hand/right/input/b/click")?,
    ));
    bindings.push(xr::Binding::new(
        &ride,
        path("/user/hand/left/input/x/click")?,
    ));
    bindings.push(xr::Binding::new(
        &next,
        path("/user/hand/left/input/y/click")?,
    ));
    bindings.push(xr::Binding::new(
        &menu,
        path("/user/hand/left/input/menu/click")?,
    ));
    instance.suggest_interaction_profile_bindings(
        path("/interaction_profiles/oculus/touch_controller")?,
        &bindings,
    )?;
    session.attach_action_sets(&[&set])?;
    let space =
        |a: &xr::Action<xr::Posef>, hand| a.create_space(session, hand, xr::Posef::IDENTITY);
    Ok(Controls {
        set,
        select,
        squeeze,
        stick,
        next,
        pause,
        ride,
        menu,
        aims: [space(&aim, hands[0])?, space(&aim, hands[1])?],
        grips: [space(&grip_pose, hands[0])?, space(&grip_pose, hands[1])?],
        hands,
    })
}

/// Whether the game draws for a headset; starts VR the first time it is
/// asked, making the GPU too.
pub(crate) fn active(w: &mut World) -> bool {
    let r = &mut w.renderer;
    if matches!(r.xr, State::Untried) {
        r.xr = match start(w.profile.on) {
            Some((xr, gpu)) => {
                r.set_gpu(gpu);
                State::On(xr)
            }
            None => State::Off,
        };
    }
    matches!(r.xr, State::On(_))
}

/// Where `space` is at `time` in the headset's space, if tracked.
fn locate(space: &xr::Space, base: &xr::Space, time: xr::Time) -> Option<(Vec3, Quat)> {
    let loc = space.locate(base, time).ok()?;
    loc.location_flags
        .contains(
            xr::SpaceLocationFlags::POSITION_VALID | xr::SpaceLocationFlags::ORIENTATION_VALID,
        )
        .then(|| pose(loc.pose))
}

/// Takes in the controllers, at the start of a frame: grabbing moves, turns
/// and sizes the level; the sticks walk, turn and rise; A moves on
/// between the level's camera views; and the pointing hand's ray is the
/// mouse, on the UI or into the level, its trigger the left button.
pub(crate) fn input(w: &mut World) {
    if !active(w) {
        return;
    }
    let following = w.lemming_cam.following().is_some();
    let now = w.e.time.real.elapsed_secs();
    let State::On(x) = &mut w.renderer.xr else {
        return;
    };
    if let Some(key) = x.key.take() {
        w.e.input.keys.release(key);
    }
    // Where the level is: at the game's camera, whose position and heading
    // the player's walking, turning and grabbing move (so that each camera
    // view keeps where the player leaves it); life-size afresh with each
    // level, the lemming view and play.
    let playing = w.state == crate::menu::AppState::Playing;
    let placed_for = (w.current.loaded, following, playing);
    let mpu = if x.placed_for == placed_for {
        x.place.mpu
    } else {
        METRES_PER_UNIT
    };
    x.placed_for = placed_for;
    x.place = Place {
        mpu,
        ..Place::at_view(&w.view)
    };
    // Cutting: the game's glide ends at once, so the level need not wait
    // for it.
    if let Some(start) = x.jump {
        w.view.finish_glide();
        if now - start >= JUMP_DARK {
            x.jump = None;
        }
    }
    if let Some(start) = x.gliding
        && w.view.glide.is_none()
        && now - start >= GLIDE_START
    {
        x.gliding = None;
    }
    // Squeezes count only while they are read.
    w.game.squeeze = [0.0; 2];
    let (Some(time), true) = (x.time, x.focused) else {
        return;
    };
    if x.session
        .sync_actions(&[xr::ActiveActionSet::new(&x.controls.set)])
        .is_err()
    {
        return;
    }
    let c = &x.controls;
    let session = &x.session;
    let held = |a: &xr::Action<bool>, hand| {
        a.state(session, hand)
            .is_ok_and(|s| s.is_active && s.current_state)
    };
    let went_down = |a: &xr::Action<bool>| {
        a.state(session, xr::Path::NULL)
            .is_ok_and(|s| s.is_active && s.current_state && s.changed_since_last_sync)
    };
    let pressed = [held(&c.select, c.hands[0]), held(&c.select, c.hands[1])];
    let squeezed = [0, 1].map(|h| {
        c.squeeze
            .state(session, c.hands[h])
            .ok()
            .filter(|s| s.is_active)
            .map_or(0.0, |s| s.current_state)
    });
    let rays = [0, 1].map(|h| locate(&c.aims[h], &x.space, time));
    let stick = |hand: usize| {
        c.stick
            .state(session, c.hands[hand])
            .ok()
            .filter(|s| s.is_active)
            .map_or(Vec2::ZERO, |s| {
                Vec2::new(s.current_state.x, s.current_state.y)
            })
    };
    let (walk, turn) = (stick(0), stick(1));
    let next_view = went_down(&c.next);
    let pause = went_down(&c.pause);
    let ride = went_down(&c.ride);
    let menu = went_down(&c.menu);
    let head = session
        .locate_views(VIEW_TYPE, time, &x.space)
        .ok()
        .map(|(_, v)| pose(v[0].pose));

    // The menu and pausing: the game's own keys.
    for (down, key) in [(menu, KeyCode::Escape), (pause, KeyCode::KeyP)] {
        if down && x.key.is_none() {
            w.e.input.keys.press(key);
            x.key = Some(key);
        }
    }
    // Time: the squeezes are a gamepad's triggers (`backspace_rewind`), the
    // left running the level back and the right ahead, as fast as squeezed.
    w.game.squeeze = squeezed;

    // A camera view: the game's own keys, in the dark.
    if next_view && !following && x.jump.is_none() && x.gliding.is_none() && x.key.is_none() {
        let next = (w.preset.0 + 1) % 4;
        let key = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
        ][next];
        w.e.input.keys.press(key);
        x.key = Some(key);
        if w.settings.vr_glide {
            x.gliding = Some(now);
        } else {
            x.jump = Some(now);
        }
    }

    let place = &mut x.place;

    // The sticks: walking where the head looks, snap turns about the head,
    // rising and sinking; on the level list the right stick scrolls it
    // instead of rising.
    let dt = w.e.time.real.delta_secs();
    let listing = w.state == crate::menu::AppState::Menu;
    if listing && turn.y.abs() > STICK_DEAD && turn.y.abs() > turn.x.abs() {
        use l3d_engine::input::{MouseScrollUnit, MouseWheel};
        w.e.input.wheel.push(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: turn.y * LIST_ROWS_PER_SEC * dt,
        });
    }
    if walk.length() > STICK_DEAD {
        let look = head.map_or(Quat::IDENTITY, |h| h.1);
        let ahead = (anchor_rotation(place.yaw) * look * Vec3::NEG_Z)
            .with_y(0.0)
            .normalize_or(Vec3::X);
        let right = Vec3::new(-ahead.z, 0.0, ahead.x);
        place.pos += (ahead * walk.y + right * walk.x) * crate::CAMERA_SPEED * dt;
    }
    let rising = !listing && turn.y.abs() > STICK_DEAD && turn.y.abs() > turn.x.abs();
    if rising {
        place.pos.y += turn.y * RISE_SPEED * dt;
    }
    // Turning about the head, in steps or (Smooth Turn) smoothly; a push
    // right turns right: the heading's yaw goes up.
    let smooth = w.settings.smooth_turn;
    let turning = turn.x.abs() > STICK_DEAD && turn.x.abs() > turn.y.abs();
    let angle = if turn.x.abs() < STICK_DEAD {
        x.turn_ready = true;
        0.0
    } else if smooth && turning {
        turn.x * SMOOTH_TURN_SPEED * dt
    } else if x.turn_ready && turning {
        x.turn_ready = false;
        SNAP_TURN * turn.x.signum()
    } else {
        0.0
    };
    if angle != 0.0 {
        let at = head.map_or(Vec3::ZERO, |h| h.0);
        let before = place.world_from_space().transform_point3(at);
        place.yaw += angle;
        let after = place.world_from_space().transform_point3(at);
        place.pos += before - after;
    }
    // The comfort vignette comes in while the sticks move the player
    // smoothly (not for a step's turn), and goes again after.
    let moving = walk.length() > STICK_DEAD || rising || (smooth && turning);
    let target = (w.settings.vr_vignette && moving) as u8 as f32;
    let step = dt / VIGNETTE_FADE;
    x.vignette += (target - x.vignette).clamp(-step, step);
    // The game's camera follows (not in the lemming view, which is the
    // lemming's, or while moving to another view).
    if !following && x.jump.is_none() && x.gliding.is_none() {
        (w.view.pos, w.view.yaw) = (x.place.pos, x.place.yaw);
    }

    // Pointing: the hand clicking, else the one whose trigger went down
    // last.
    if x.clicking.is_none()
        && let Some(hand) = (0..2).find(|&h| pressed[h] && rays[h].is_some())
    {
        x.hand = hand;
    }
    if rays[x.hand].is_none() {
        x.hand = 1 - x.hand;
    }
    x.aim = None;
    let mut under_ray = None;
    x.beam = None;
    let panel = if x.panel_pose.palette {
        crate::panel::palette_pixels(&w.screen)
    } else if x.panel_pose.popup {
        crate::confirm::popup_pixels(&w.confirm, &w.screen)
    } else {
        w.screen.pixels
    };
    let window = &mut w.e.window;
    match rays[x.hand] {
        Some((origin, rotation)) => {
            let dir = rotation * Vec3::NEG_Z;
            let on_ui = panel_hit(&x.panel_pose, origin, dir, panel).filter(|(uv, _)| {
                // The palette's empty parts let the ray through.
                !x.panel_pose.palette
                    || w.ui
                        .shows_at(*uv * Vec2::new(panel.z as f32, panel.w as f32))
            });
            if let Some((uv, at)) = on_ui {
                let p = Vec2::new(panel.x as f32, panel.y as f32)
                    + uv * Vec2::new(panel.z as f32, panel.w as f32);
                // Outside levels (where it would drag the camera) the
                // pointer's moving is the mouse's: it highlights what it is
                // on, as in the level list.
                if !playing && let Some(before) = window.physical_cursor_position() {
                    w.e.input.motion.delta += (p - before) / window.scale_factor();
                }
                window.set_physical_cursor_position(Some(p));
                x.beam = Some(Beam {
                    start: origin,
                    end: at,
                    colour: BEAM_PANEL,
                });
            } else {
                // Into the level: the game sees it as a view along the ray
                // with the pointer in its middle.
                let anchor = x.place.world_from_space();
                let from = anchor.transform_point3(origin);
                let toward = anchor.transform_vector3(dir).normalize();
                let up = anchor.transform_vector3(rotation * Vec3::Y).normalize();
                let size = w.screen.size;
                let view_proj = crate::projection(AIM_FOV_Y, size.x / size.y.max(1.0), 0.5)
                    * Mat4::look_to_rh(from, toward, up);
                x.aim = Some(view_proj);
                window.set_physical_cursor_position(Some(Vec2::new(
                    (panel.x + panel.z / 2) as f32,
                    (panel.y + panel.w / 2) as f32,
                )));
                let aim_camera = SceneCamera {
                    view_proj,
                    ..w.camera
                };
                let picked = w.game.sim.as_ref().and_then(|sim| {
                    let i = crate::hud::lemming_at(sim, &aim_camera, size, size / 2.0)?;
                    under_ray = Some(i);
                    Some(Vec3::from_array(sim.lemmings[i].pos.map(crate::units)) + Vec3::Y * 0.2)
                });
                let solid = || {
                    let (level, ..) = w.game.terrain.as_ref()?;
                    crate::first_solid(level, from, toward).or_else(|| {
                        let t = (crate::scene_build::GROUND_Y - from.y) / toward.y;
                        (t > 0.0).then(|| from + toward * t)
                    })
                };
                let space_from_world = anchor.inverse();
                let end = picked
                    .or_else(solid)
                    .map(|p| space_from_world.transform_point3(p))
                    .filter(|p| p.distance(origin) < BEAM_LONGEST)
                    .unwrap_or(origin + dir * BEAM_LONGEST);
                x.beam = Some(Beam {
                    start: origin,
                    end,
                    colour: if picked.is_some() {
                        BEAM_LEMMING
                    } else {
                        BEAM_LEVEL
                    },
                });
            }
        }
        None if x.clicking.is_none() => window.set_physical_cursor_position(None),
        None => {}
    }
    let world_from_space = x.place.world_from_space();
    x.seen_from = head.map(|(position, rotation)| {
        let forward = world_from_space.transform_vector3(rotation * Vec3::NEG_Z);
        (
            world_from_space.transform_point3(position),
            (-forward.z).atan2(-forward.x),
        )
    });
    let mouse = &mut w.e.input.mouse;
    if x.clicking.is_none() && pressed[x.hand] && rays[x.hand].is_some() {
        x.clicking = Some(x.hand);
        mouse.press(MouseButton::Left);
    }
    if let Some(hand) = x.clicking
        && !pressed[hand]
    {
        x.clicking = None;
        mouse.release(MouseButton::Left);
    }
    // X: riding along with the lemming under the ray, as a gamepad's X with
    // the highlighted one (the skill stays selected); again, or with none
    // under the ray, the lemming view's own toggle.
    if ride {
        let selected = (w.selected.0, w.selected.1);
        let (cam, highlight, chosen, view) = (
            &mut w.lemming_cam,
            &mut w.highlight,
            &mut w.selected,
            &mut w.view,
        );
        match under_ray {
            Some(i) if cam.following() != Some(i) => cam.ride(i, highlight, chosen, view),
            _ => cam.face(highlight, chosen, view),
        }
        (w.selected.0, w.selected.1) = selected;
    }
    aim(w);
}

/// Where the head is in the level, in units, and the way it faces (the
/// camera-yaw convention), while in the headset: what sprites turn to.
pub(crate) fn seen_from(w: &World) -> Option<(Vec3, f32)> {
    match &w.renderer.xr {
        State::On(x) => x.seen_from,
        _ => None,
    }
}

/// Points the game's camera along the controller's ray while it points
/// into the level, so that what the pointer picks is what the ray meets.
/// After the camera's own update, and at the start of the frame.
pub(crate) fn aim(w: &mut World) {
    if let State::On(x) = &w.renderer.xr
        && let Some(view_proj) = x.aim
    {
        w.camera.view_proj = view_proj;
    }
}

/// Draws the frame for the headset: the level for each eye, and the UI on
/// its panel.
pub(crate) fn render(w: &mut World) {
    // While playing, the UI is the palette on the left hand (but a level's
    // menu or preview floats ahead, whole).
    let palette = w.state == crate::menu::AppState::Playing
        && w.confirm.question.is_none()
        && !w.preview.active;
    let on = matches!(w.renderer.xr, State::On(_));
    // A turner waiting for its side: its arrows, drawn in the level.
    let aim = match &w.renderer.xr {
        State::On(x) => x.aim,
        _ => None,
    };
    let aim_camera = aim.map(|view_proj| SceneCamera {
        view_proj,
        ..w.camera
    });
    let turner = crate::hud::pending_turner_sides(w, aim_camera.as_ref());
    w.panel.palette = palette && on;
    // The level's menu pops up ahead instead.
    let popup = w.state == crate::menu::AppState::Playing && w.confirm.question.is_some();
    w.confirm.popup = popup && on;
    let palette_px = crate::panel::palette_pixels(&w.screen);
    let popup_px = crate::confirm::popup_pixels(&w.confirm, &w.screen);
    w.time_overlay.vr = on;
    let dial_px = crate::time_dial::band_pixels(&w.time_overlay, &w.screen);
    let (Some(gpu), State::On(x)) = w.renderer.gpu_and_xr() else {
        return;
    };
    // The session's changes.
    while let Ok(Some(event)) = x.instance.poll_event(&mut x.events) {
        match event {
            xr::Event::SessionStateChanged(e) => {
                info!("VR session {:?}", e.state());
                match e.state() {
                    xr::SessionState::READY => {
                        if let Err(e) = x.session.begin(VIEW_TYPE) {
                            error!("VR session didn't start: {e}");
                        }
                        x.running = true;
                    }
                    xr::SessionState::STOPPING => {
                        let _ = x.session.end();
                        x.running = false;
                    }
                    xr::SessionState::FOCUSED => x.focused = true,
                    xr::SessionState::VISIBLE => x.focused = false,
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
            xr::Event::InstanceLossPending(_) => std::process::exit(0),
            _ => {}
        }
    }
    if !x.running {
        std::thread::sleep(std::time::Duration::from_millis(20));
        return;
    }
    let before_wait = Instant::now();
    let Ok(state) = x.waiter.wait() else {
        return;
    };
    let after_wait = Instant::now();
    if x.stream.begin().is_err() {
        return;
    }
    x.time = Some(state.predicted_display_time);
    if !state.should_render {
        let _ = x.stream.end(state.predicted_display_time, x.blend, &[]);
        return;
    }
    let Ok((_, views)) = x
        .session
        .locate_views(VIEW_TYPE, state.predicted_display_time, &x.space)
    else {
        let _ = x.stream.end(state.predicted_display_time, x.blend, &[]);
        return;
    };
    let device = &gpu.device;
    if x.eyes.is_none() {
        let v = x
            .instance
            .enumerate_view_configuration_views(x.system, VIEW_TYPE)
            .expect("the views");
        let size = UVec2::new(
            v[0].recommended_image_rect_width,
            v[0].recommended_image_rect_height,
        );
        info!("VR eyes {size}");
        x.eyes = Some(chain(&x.session, device, size, 2));
    }
    let panel_px = UVec2::new(w.screen.pixels.z, w.screen.pixels.w).max(UVec2::ONE);
    if x.panel.as_ref().is_none_or(|p| p.size != panel_px) {
        x.panel = None;
        x.panel = Some(chain(&x.session, device, panel_px, 1));
    }

    // The eyes.
    let eyes = x.eyes.as_mut().expect("made above");
    let image = eyes.handle.acquire_image().expect("an eye image") as usize;
    let _ = eyes.handle.wait_image(xr::Duration::INFINITE);
    let anchor = x.place.world_from_space();
    let size = eyes.size;
    for (eye, view) in views.iter().enumerate() {
        let camera = eye_camera(&w.camera, anchor, view);
        let target = &eyes.images[image].1[eye];
        let mut encoder = device.create_command_encoder(&Default::default());
        if x.jump.is_some() {
            // Dark while jumping to a camera view.
            clear(&mut encoder, target, LinearRgba::new(0.0, 0.0, 0.0, 1.0));
            gpu.queue.submit([encoder.finish()]);
            continue;
        }
        let c = w.clear.0.to_linear();
        x.scene.draw(
            gpu,
            &mut encoder,
            target,
            size,
            UVec4::new(0, 0, size.x, size.y),
            &SceneInput {
                content: &w.scene,
                sprites: &w.sprites,
                camera: &camera,
                profile: &w.profile,
                format: TARGET_FORMAT,
                headroom: 0.0,
                screen: None,
                // The fields' changes are taken by the first eye; the
                // second draws them as they are then.
                signals: w.signals.as_mut().map(|f| {
                    let dirty = f.dirty.take();
                    (&*f, dirty)
                }),
                marks: w.marks.as_mut().map(|f| {
                    let dirty = f.dirty.take();
                    (&*f, dirty)
                }),
                clear: wgpu::Color {
                    r: c.red as f64,
                    g: c.green as f64,
                    b: c.blue as f64,
                    a: c.alpha as f64,
                },
                overlay: None,
            },
        );
        // Each eye on its own: the scene's uniforms are written for each.
        gpu.queue.submit([encoder.finish()]);
    }
    let _ = eyes.handle.release_image();

    // The panel.
    let panel = x.panel.as_mut().expect("made above");
    let image = panel.handle.acquire_image().expect("a panel image") as usize;
    let _ = panel.handle.wait_image(xr::Duration::INFINITE);
    {
        let target = &panel.images[image].1[0];
        let mut encoder = device.create_command_encoder(&Default::default());
        clear(&mut encoder, target, LinearRgba::new(0.0, 0.0, 0.0, 0.0));
        x.ui.get_or_insert_with(|| UiGpu::new(gpu, TARGET_FORMAT, None))
            .draw(
                gpu,
                &mut encoder,
                target,
                UVec4::new(0, 0, panel.size.x, panel.size.y),
                &w.ui,
                &mut w.e.images,
            );
        gpu.queue.submit([encoder.finish()]);
    }
    let _ = panel.handle.release_image();

    // A waiting turner's arrows, in the headset's space.
    let space_from_world = x.place.world_from_space().inverse();
    let arrows: Vec<(Vec3, Vec4)> = turner
        .map(|(centre, dirs, chosen)| turner_arrows(centre, dirs, chosen))
        .unwrap_or_default()
        .into_iter()
        .map(|(p, colour)| (space_from_world.transform_point3(p), colour))
        .collect();
    // The ray and the arrows, over everything.
    let eye_size = x.eyes.as_ref().expect("made above").size;
    let overlay = x.overlay.get_or_insert_with(|| Overlay {
        chain: chain(&x.session, device, eye_size / OVERLAY_SCALE, 2),
        pipeline: overlay_pipeline(device),
    });
    let image = overlay
        .chain
        .handle
        .acquire_image()
        .expect("an overlay image") as usize;
    let _ = overlay.chain.handle.wait_image(xr::Duration::INFINITE);
    {
        let mut encoder = device.create_command_encoder(&Default::default());
        for (eye, view) in views.iter().enumerate() {
            let target = &overlay.chain.images[image].1[eye];
            let mut vertices = vignette_vertices(x.vignette);
            vertices.extend(triangle_vertices(&arrows, view));
            vertices.extend(x.beam.map(|b| beam_vertices(&b, view)).unwrap_or_default());
            let buffer = (!vertices.is_empty()).then(|| {
                wgpu::util::DeviceExt::create_buffer_init(
                    device,
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("VR ray"),
                        contents: &vertices
                            .iter()
                            .flat_map(|v| v.to_le_bytes())
                            .collect::<Vec<u8>>(),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                )
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("VR ray"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if let Some(buffer) = &buffer {
                pass.set_pipeline(&overlay.pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..(vertices.len() / VERTEX_FLOATS) as u32, 0..1);
            }
        }
        gpu.queue.submit([encoder.finish()]);
    }
    let _ = overlay.chain.handle.release_image();

    let rect = |size: UVec2| xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: size.x as i32,
            height: size.y as i32,
        },
    };
    let eyes = x.eyes.as_ref().expect("made above");
    let projection_views = [0, 1].map(|i| {
        xr::CompositionLayerProjectionView::new()
            .pose(views[i].pose)
            .fov(views[i].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&eyes.handle)
                    .image_array_index(i as u32)
                    .image_rect(rect(eyes.size)),
            )
    });
    let projection = xr::CompositionLayerProjection::new()
        .space(&x.space)
        .views(&projection_views);
    let panel = x.panel.as_ref().expect("made above");
    if !popup {
        x.popup_at = None;
    } else if x.popup_at.is_none() {
        x.popup_at = Some(PanelPose {
            popup: true,
            ..ahead_of_head(&views, POPUP_AHEAD, POPUP_BELOW, POPUP_TILT, POPUP_WIDTH)
        });
    }
    if dial_px.is_none() {
        x.dial_at = None;
    } else if x.dial_at.is_none() {
        x.dial_at = Some(ahead_of_head(&views, DIAL_AHEAD, 0.0, 0.0, DIAL_WIDTH));
    }
    x.panel_pose = if let Some(at) = &x.popup_at {
        *at
    } else {
        palette
            .then(|| locate(&x.controls.grips[0], &x.space, state.predicted_display_time))
            .flatten()
            .map_or_else(front_panel, |(hand, turn)| PanelPose {
                centre: hand + turn * PALETTE_OFFSET,
                rotation: turn * Quat::from_rotation_x(PALETTE_TILT),
                width: PALETTE_WIDTH,
                palette: true,
                popup: false,
            })
    };
    let PanelPose {
        centre,
        rotation,
        width,
        palette: palette_on,
        popup: popup_on,
    } = x.panel_pose;
    // The part of the UI shown: the palette's strip, the popup's card, or
    // all of it.
    let part = if palette_on {
        Some(palette_px)
    } else if popup_on {
        Some(popup_px)
    } else {
        None
    };
    let quad = xr::CompositionLayerQuad::new()
        .space(&x.space)
        .layer_flags(xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
        .eye_visibility(xr::EyeVisibility::BOTH)
        .sub_image(
            xr::SwapchainSubImage::new()
                .swapchain(&panel.handle)
                .image_rect(match part {
                    Some(p) => xr::Rect2Di {
                        offset: xr::Offset2Di {
                            x: p.x as i32,
                            y: p.y as i32,
                        },
                        extent: xr::Extent2Di {
                            width: p.z.min(panel.size.x.saturating_sub(p.x)) as i32,
                            height: p.w.min(panel.size.y.saturating_sub(p.y)) as i32,
                        },
                    },
                    None => rect(panel.size),
                }),
        )
        .pose(xr::Posef {
            orientation: xr::Quaternionf {
                x: rotation.x,
                y: rotation.y,
                z: rotation.z,
                w: rotation.w,
            },
            position: xr::Vector3f {
                x: centre.x,
                y: centre.y,
                z: centre.z,
            },
        })
        .size(xr::Extent2Df {
            width,
            height: match part {
                Some(p) => width * p.w as f32 / p.z.max(1) as f32,
                None => width * panel.size.y as f32 / panel.size.x.max(1) as f32,
            },
        });
    let overlay = x.overlay.as_ref().expect("made above");
    let overlay_views = [0, 1].map(|i| {
        xr::CompositionLayerProjectionView::new()
            .pose(views[i].pose)
            .fov(views[i].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&overlay.chain.handle)
                    .image_array_index(i as u32)
                    .image_rect(rect(overlay.chain.size)),
            )
    });
    let ray = xr::CompositionLayerProjection::new()
        .space(&x.space)
        .layer_flags(xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
        .views(&overlay_views);
    // The time control's bar, from the same UI image.
    let dial = x.dial_at.zip(dial_px).map(|(at, p)| {
        xr::CompositionLayerQuad::new()
            .space(&x.space)
            .layer_flags(xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
            .eye_visibility(xr::EyeVisibility::BOTH)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&panel.handle)
                    .image_rect(xr::Rect2Di {
                        offset: xr::Offset2Di {
                            x: p.x as i32,
                            y: p.y as i32,
                        },
                        extent: xr::Extent2Di {
                            width: p.z.min(panel.size.x.saturating_sub(p.x)) as i32,
                            height: p.w.min(panel.size.y.saturating_sub(p.y)) as i32,
                        },
                    }),
            )
            .pose(posef(at.centre, at.rotation))
            .size(xr::Extent2Df {
                width: at.width,
                height: at.width * p.w as f32 / p.z.max(1) as f32,
            })
    });
    let level = w.current.loaded;
    x.timing.frame(before_wait, after_wait, level);
    let mut layers: Vec<&xr::CompositionLayerBase<xr::Vulkan>> = vec![&projection, &quad];
    if let Some(d) = &dial {
        layers.push(d);
    }
    layers.push(&ray);
    if let Err(e) = x.stream.end(state.predicted_display_time, x.blend, &layers) {
        warn!("VR frame: {e}");
    }
}

/// Where the frames' time goes on the CPU, summed and logged every
/// [`TIMING_SECONDS`]: the game's own work between frames, the wait for the
/// headset, and drawing (recording and submitting the GPU's work).
#[derive(Default)]
struct Timing {
    since: Option<Instant>,
    last_end: Option<Instant>,
    frames: u32,
    game: f32,
    waiting: f32,
    drawing: f32,
}

const TIMING_SECONDS: f32 = 2.0;

impl Timing {
    fn frame(&mut self, before_wait: Instant, after_wait: Instant, level: Option<u32>) {
        let now = Instant::now();
        let since = *self.since.get_or_insert(now);
        if let Some(end) = self.last_end {
            self.game += (before_wait - end).as_secs_f32();
        }
        self.waiting += (after_wait - before_wait).as_secs_f32();
        self.drawing += (now - after_wait).as_secs_f32();
        self.frames += 1;
        self.last_end = Some(now);
        let span = (now - since).as_secs_f32();
        if span >= TIMING_SECONDS {
            let ms = |t: f32| t * 1000.0 / self.frames as f32;
            info!(
                "VR timing, level {level:?}: {:.1} fps; per frame {:.1} ms game, {:.1} ms drawing, {:.1} ms waiting",
                self.frames as f32 / span,
                ms(self.game),
                ms(self.drawing),
                ms(self.waiting),
            );
            *self = Timing {
                last_end: self.last_end,
                since: Some(now),
                ..Timing::default()
            };
        }
    }
}

/// A swapchain of `layers` layers of `size`, its images wrapped for wgpu.
fn chain(
    session: &xr::Session<xr::Vulkan>,
    device: &wgpu::Device,
    size: UVec2,
    layers: u32,
) -> Chain {
    let handle = session
        .create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED,
            format: SWAPCHAIN_FORMAT.as_raw() as _,
            sample_count: 1,
            width: size.x,
            height: size.y,
            face_count: 1,
            array_size: layers,
            mip_count: 1,
        })
        .expect("a VR swapchain");
    let extent = wgpu::Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: layers,
    };
    let images = handle
        .enumerate_images()
        .expect("the swapchain's images")
        .into_iter()
        .map(|raw| {
            let hal_desc = hal::TextureDescriptor {
                label: Some("VR image"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET_FORMAT,
                usage: wgpu::TextureUses::COLOR_TARGET,
                memory_flags: hal::MemoryFlags::empty(),
                view_formats: vec![],
            };
            let texture = unsafe {
                let hal_device = device
                    .as_hal::<hal::api::Vulkan>()
                    .expect("a Vulkan device");
                let hal_texture = hal_device.texture_from_raw(
                    vk::Image::from_raw(raw),
                    &hal_desc,
                    // The swapchain owns the image.
                    Some(Box::new(|| {})),
                    hal::vulkan::TextureMemory::External,
                );
                device.create_texture_from_hal::<hal::api::Vulkan>(
                    hal_texture,
                    &wgpu::TextureDescriptor {
                        label: Some("VR image"),
                        size: extent,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: TARGET_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    },
                )
            };
            let views = (0..layers)
                .map(|layer| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            (texture, views)
        })
        .collect();
    Chain {
        handle,
        size,
        images,
    }
}

fn clear(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, c: LinearRgba) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("VR clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
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
}

/// An OpenXR pose as a position and a rotation.
fn pose(p: xr::Posef) -> (Vec3, Quat) {
    (
        Vec3::new(p.position.x, p.position.y, p.position.z),
        Quat::from_xyzw(
            p.orientation.x,
            p.orientation.y,
            p.orientation.z,
            p.orientation.w,
        ),
    )
}

/// The turn taking the head's starting forward (−Z) to the camera's
/// heading at `yaw` (see `ViewCamera::forward`).
fn anchor_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - yaw)
}

/// The scene camera for an eye at `view`, the rest as the game's `base`.
fn eye_camera(base: &SceneCamera, anchor: Mat4, view: &xr::View) -> SceneCamera {
    let (position, rotation) = pose(view.pose);
    let world_from_eye = anchor * Mat4::from_rotation_translation(rotation, position);
    let view_proj = fov_projection(view.fov) * world_from_eye.inverse();
    let right = world_from_eye.transform_vector3(Vec3::X).normalize();
    // The sky all round, fixed in the world: its panorama as high above
    // the horizon as the screen shows it.
    let top = crate::DEFAULT_HORIZON * 2.0 * (crate::DEFAULT_FOV_Y.to_radians() / 2.0).tan();
    SceneCamera {
        view_proj,
        horizon: 0.5,
        sky_column: 0.0,
        dome: top.atan(),
        halos: true,
        right,
        roll: 0.0,
        pixels: UVec2::ZERO,
        ..*base
    }
}

/// An eye's projection with reversed depth to infinity (wgpu's clip space,
/// the near plane at depth 1), for the asymmetric field of view `fov`.
fn fov_projection(fov: xr::Fovf) -> Mat4 {
    let (l, r) = (fov.angle_left.tan(), fov.angle_right.tan());
    let (d, u) = (fov.angle_down.tan(), fov.angle_up.tan());
    Mat4::from_cols(
        Vec4::new(2.0 / (r - l), 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / (u - d), 0.0, 0.0),
        Vec4::new((r + l) / (r - l), (u + d) / (u - d), 0.0, -1.0),
        Vec4::new(0.0, 0.0, NEAR, 0.0),
    )
}

/// A turner's two arrows, flat beside the lemming at `centre` pointing to
/// its sides `dirs` (level units): the one a click would pick bright, the
/// other faint; both half bright while neither is clearly meant. White with
/// a dark rim, as the panel's arrows.
fn turner_arrows(centre: Vec3, dirs: [Vec3; 2], chosen: Option<usize>) -> Vec<(Vec3, Vec4)> {
    let mut out = Vec::new();
    for (k, d) in dirs.into_iter().enumerate() {
        let alpha = match chosen {
            Some(c) if c == k => 1.0,
            Some(_) => 0.2,
            None => 0.6,
        };
        let across = Vec3::new(-d.z, 0.0, d.x);
        for (grow, colour) in [
            (0.05, Vec4::new(0.0, 0.0, 0.0, 0.7 * alpha)),
            (0.0, Vec4::new(1.0, 1.0, 1.0, alpha)),
        ] {
            let (base, tip) = (centre + d * (0.3 - grow), centre + d * (0.85 + grow));
            let neck = tip - d * (0.25 + grow);
            let (shaft, head) = (0.06 + grow, 0.18 + grow * 1.5);
            out.extend(
                [
                    base - across * shaft,
                    base + across * shaft,
                    neck + across * shaft,
                    base - across * shaft,
                    neck + across * shaft,
                    neck - across * shaft,
                    neck - across * head,
                    neck + across * head,
                    tip,
                ]
                .map(|v| (v, colour)),
            );
        }
    }
    out
}

/// The comfort vignette, `k` of the way in, for the overlay: a dark ring
/// round the view, straight in clip space (the same for each eye).
fn vignette_vertices(k: f32) -> Vec<f32> {
    if k <= 0.01 {
        return Vec::new();
    }
    const SEGMENTS: usize = 48;
    let dark = k * VIGNETTE_DARK;
    // Clear inside, then darker to the edge, then dark past the corners.
    let rings = [(VIGNETTE_INNER, 0.0), (VIGNETTE_EDGE, dark), (2.0, dark)];
    let mut out = Vec::with_capacity(SEGMENTS * 12 * VERTEX_FLOATS);
    let mut corner = |r: f32, angle: f32, alpha: f32| {
        out.extend([r * angle.cos(), r * angle.sin(), 0.0, 1.0]);
        out.extend([0.0, 0.0, 0.0, alpha]);
        out.extend([0.0, 0.0]);
    };
    for i in 0..SEGMENTS {
        let step = std::f32::consts::TAU / SEGMENTS as f32;
        let (a0, a1) = (step * i as f32, step * (i + 1) as f32);
        for pair in rings.windows(2) {
            let [(ri, ai), (ro, ao)] = [pair[0], pair[1]];
            corner(ri, a0, ai);
            corner(ro, a0, ao);
            corner(ro, a1, ao);
            corner(ri, a0, ai);
            corner(ro, a1, ao);
            corner(ri, a1, ai);
        }
    }
    out
}

/// Triangles (corners in the headset's space, with colours) for the
/// overlay, as eye `view` sees them.
fn triangle_vertices(corners: &[(Vec3, Vec4)], view: &xr::View) -> Vec<f32> {
    let (eye, rotation) = pose(view.pose);
    let to_clip =
        fov_projection(view.fov) * Mat4::from_rotation_translation(rotation, eye).inverse();
    let mut out = Vec::with_capacity(corners.len() * VERTEX_FLOATS);
    for &(p, colour) in corners {
        out.extend(to_clip.mul_vec4(p.extend(1.0)).to_array());
        out.extend(colour.to_array());
        out.extend([0.0, 0.0]);
    }
    out
}

/// A panel `ahead` metres ahead of the head and `below` under its height,
/// level, facing it and leaning back by `tilt` radians, `width` wide.
fn ahead_of_head(views: &[xr::View], ahead: f32, below: f32, tilt: f32, width: f32) -> PanelPose {
    let (a, b) = (pose(views[0].pose), pose(views[1].pose));
    let head = (a.0 + b.0) / 2.0;
    let forward = (a.1 * Vec3::NEG_Z).with_y(0.0).normalize_or(Vec3::NEG_Z);
    let turn = Quat::from_rotation_y((-forward.x).atan2(-forward.z));
    PanelPose {
        centre: head + forward * ahead - Vec3::Y * below,
        rotation: turn * Quat::from_rotation_x(-tilt),
        width,
        palette: false,
        popup: false,
    }
}

/// An OpenXR pose from a position and rotation.
fn posef(position: Vec3, rotation: Quat) -> xr::Posef {
    xr::Posef {
        orientation: xr::Quaternionf {
            x: rotation.x,
            y: rotation.y,
            z: rotation.z,
            w: rotation.w,
        },
        position: xr::Vector3f {
            x: position.x,
            y: position.y,
            z: position.z,
        },
    }
}

/// The UI floating ahead, where the head began.
fn front_panel() -> PanelPose {
    PanelPose {
        centre: PANEL_CENTRE,
        rotation: Quat::from_rotation_x(-PANEL_TILT),
        width: PANEL_WIDTH,
        palette: false,
        popup: false,
    }
}

/// Where a ray from `origin` along `dir` meets the UI shown at `at`, an
/// image of the screen `panel` (its pixels' rectangle), as 0–1 across and
/// down, and the point met; none if it misses.
fn panel_hit(at: &PanelPose, origin: Vec3, dir: Vec3, panel: UVec4) -> Option<(Vec2, Vec3)> {
    let normal = at.rotation * Vec3::Z;
    let facing = dir.dot(normal);
    if facing >= -1e-4 {
        return None;
    }
    let t = (at.centre - origin).dot(normal) / facing;
    if t <= 0.0 {
        return None;
    }
    let hit = origin + dir * t;
    let local = at.rotation.inverse() * (hit - at.centre);
    let height = at.width * panel.w as f32 / panel.z.max(1) as f32;
    let uv = Vec2::new(local.x / at.width + 0.5, 0.5 - local.y / height);
    (uv.cmpge(Vec2::ZERO).all() && uv.cmple(Vec2::ONE).all()).then_some((uv, hit))
}

/// Floats per vertex of the ray: clip position, colour (premultiplied
/// below), and where in a dot it is (−1–1; 0 along the beam).
const VERTEX_FLOATS: usize = 10;

const OVERLAY_SHADER: &str = "
struct In {
    @location(0) pos: vec4<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) dot: vec2<f32>,
};
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) colour: vec4<f32>,
    @location(1) dot: vec2<f32>,
};
@vertex fn vs(v: In) -> Out {
    return Out(v.pos, v.colour, v.dot);
}
@fragment fn fs(o: Out) -> @location(0) vec4<f32> {
    let r = length(o.dot);
    let a = o.colour.a * (1.0 - smoothstep(0.75, 1.0, r));
    return vec4(o.colour.rgb * a, a);
}
";

/// Draws the ray's triangles, blending premultiplied colour over the clear
/// overlay.
fn overlay_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("VR ray"),
        source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("VR ray"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("VR ray"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: (VERTEX_FLOATS * 4) as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x4, 1 => Float32x4, 2 => Float32x2
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: TARGET_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// The ray's triangles for the eye at `view`: a ribbon facing the eye from
/// the hand to the end, fading in, and a dot at the end, a dark rim round a
/// bright middle, the same size wherever it is.
fn beam_vertices(beam: &Beam, view: &xr::View) -> Vec<f32> {
    let (eye, rotation) = pose(view.pose);
    let to_clip =
        fov_projection(view.fov) * Mat4::from_rotation_translation(rotation, eye).inverse();
    let mut out = Vec::new();
    let mut corner = |p: Vec3, colour: Vec4, dot: Vec2| {
        out.extend(to_clip.mul_vec4(p.extend(1.0)).to_array());
        out.extend(colour.to_array());
        out.extend(dot.to_array());
    };
    let (a, b) = (beam.start, beam.end);
    let side = (b - a).cross(eye - a).normalize_or_zero() * BEAM_WIDTH / 2.0;
    let faint = beam.colour.with_w(0.0);
    for (p, c) in [
        (a - side, faint),
        (a + side, faint),
        (b + side, beam.colour),
        (a - side, faint),
        (b + side, beam.colour),
        (b - side, beam.colour),
    ] {
        corner(p, c, Vec2::ZERO);
    }
    let (right, up) = (rotation * Vec3::X, rotation * Vec3::Y);
    let size = b.distance(eye) * DOT_SIZE / 2.0;
    for (scale, colour) in [
        (1.0, Vec4::new(0.0, 0.0, 0.0, 0.6)),
        (0.6, beam.colour.with_w(1.0)),
    ] {
        let r = size * scale;
        for d in [
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ] {
            corner(b + (right * d.x + up * d.y) * r, colour, d);
        }
    }
    out
}
