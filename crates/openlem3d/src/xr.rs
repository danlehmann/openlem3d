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
/// Metres per grid unit: how big the level is around the player (ours, to
/// be tuned in the headset).
const METRES_PER_UNIT: f32 = 1.0;
/// The near plane, in metres.
const NEAR: f32 = 0.05;
/// The UI panel: its width in metres, its centre relative to where the head
/// began (below and ahead), and how far it leans back, in radians.
const PANEL_WIDTH: f32 = 1.2;
const PANEL_CENTRE: Vec3 = Vec3::new(0.0, -0.45, -1.0);
const PANEL_TILT: f32 = 0.5;
/// Thumbstick travel below which it counts as centred.
const STICK_DEAD: f32 = 0.2;
/// A snap turn with the right stick, in radians.
const SNAP_TURN: f32 = std::f32::consts::FRAC_PI_6;

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
    /// Whether the right stick has been back in the middle since its last
    /// snap turn.
    turn_ready: bool,
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
    stick: xr::Action<xr::Vector2f>,
    /// The left and right hands' aim.
    aims: [xr::Space; 2],
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
    scene.compile(&gpu.device);
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
        turn_ready: true,
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

/// The controllers: each hand's aim and trigger, and the thumbsticks.
fn controls(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> xr::Result<Controls> {
    let hands = [
        instance.string_to_path("/user/hand/left")?,
        instance.string_to_path("/user/hand/right")?,
    ];
    let set = instance.create_action_set("game", "Game", 0)?;
    let aim = set.create_action::<xr::Posef>("aim", "Aim", &hands)?;
    let select = set.create_action::<bool>("select", "Select", &hands)?;
    let stick = set.create_action::<xr::Vector2f>("stick", "Thumbstick", &hands)?;
    let path = |p: &str| instance.string_to_path(p);
    let mut bindings = Vec::new();
    for side in ["left", "right"] {
        bindings.push(xr::Binding::new(
            &aim,
            path(&format!("/user/hand/{side}/input/aim/pose"))?,
        ));
        bindings.push(xr::Binding::new(
            &select,
            path(&format!("/user/hand/{side}/input/trigger/value"))?,
        ));
        bindings.push(xr::Binding::new(
            &stick,
            path(&format!("/user/hand/{side}/input/thumbstick"))?,
        ));
    }
    instance.suggest_interaction_profile_bindings(
        path("/interaction_profiles/oculus/touch_controller")?,
        &bindings,
    )?;
    session.attach_action_sets(&[&set])?;
    let aims = [
        aim.create_space(session, hands[0], xr::Posef::IDENTITY)?,
        aim.create_space(session, hands[1], xr::Posef::IDENTITY)?,
    ];
    Ok(Controls {
        set,
        select,
        stick,
        aims,
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

/// Takes in the controllers, at the start of a frame: the ray meeting the
/// panel moves the pointer, the trigger presses the left button, the left
/// stick walks and the right one turns.
pub(crate) fn input(w: &mut World) {
    if !active(w) {
        return;
    }
    let State::On(x) = &mut w.renderer.xr else {
        return;
    };
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
    let panel = w.screen.pixels;
    let mut pointer = None;
    let mut pressed = [false; 2];
    for hand in 0..2 {
        pressed[hand] = c
            .select
            .state(&x.session, c.hands[hand])
            .is_ok_and(|s| s.is_active && s.current_state);
        let Ok(loc) = c.aims[hand].locate(&x.space, time) else {
            continue;
        };
        if !loc.location_flags.contains(
            xr::SpaceLocationFlags::POSITION_VALID | xr::SpaceLocationFlags::ORIENTATION_VALID,
        ) {
            continue;
        }
        let (origin, rotation) = pose(loc.pose);
        let Some(uv) = panel_hit(origin, rotation * Vec3::NEG_Z, panel) else {
            continue;
        };
        // The hand clicking keeps the pointer; else the right hand has it.
        if pointer.is_none() || x.clicking == Some(hand) || (x.clicking.is_none() && hand == 1) {
            pointer = Some((hand, uv));
        }
    }
    let window = &mut w.e.window;
    let mouse = &mut w.e.input.mouse;
    match pointer {
        Some((hand, uv)) => {
            let p = Vec2::new(panel.x as f32, panel.y as f32)
                + uv * Vec2::new(panel.z as f32, panel.w as f32);
            window.set_physical_cursor_position(Some(p));
            if x.clicking.is_none() && pressed[hand] {
                x.clicking = Some(hand);
                mouse.press(MouseButton::Left);
            }
        }
        None if x.clicking.is_none() => window.set_physical_cursor_position(None),
        None => {}
    }
    if let Some(hand) = x.clicking
        && !pressed[hand]
    {
        x.clicking = None;
        mouse.release(MouseButton::Left);
    }

    // Walking and turning.
    let stick = |hand: usize| {
        c.stick
            .state(&x.session, c.hands[hand])
            .ok()
            .filter(|s| s.is_active)
            .map_or(Vec2::ZERO, |s| {
                Vec2::new(s.current_state.x, s.current_state.y)
            })
    };
    let (walk, turn) = (stick(0), stick(1));
    let dt = w.e.time.real.delta_secs();
    if walk.length() > STICK_DEAD {
        let head = x
            .session
            .locate_views(VIEW_TYPE, time, &x.space)
            .ok()
            .map(|(_, v)| pose(v[0].pose).1)
            .unwrap_or(Quat::IDENTITY);
        let ahead = (anchor_rotation(w.view.yaw) * head * Vec3::NEG_Z).with_y(0.0);
        let ahead = ahead.normalize_or(Vec3::X);
        let right = Vec3::new(-ahead.z, 0.0, ahead.x);
        w.view.pos += (ahead * walk.y + right * walk.x) * crate::CAMERA_SPEED * dt;
    }
    if turn.x.abs() < STICK_DEAD {
        x.turn_ready = true;
    } else if x.turn_ready {
        x.turn_ready = false;
        // Turning about the head, not the anchor: the anchor moves round it.
        let head = x
            .session
            .locate_views(VIEW_TYPE, time, &x.space)
            .ok()
            .map(|(_, v)| pose(v[0].pose).0)
            .unwrap_or(Vec3::ZERO);
        let before = world_from_space(w.view.pos, w.view.yaw).transform_point3(head);
        // A push right turns right: the heading's yaw goes down.
        w.view.yaw -= SNAP_TURN * turn.x.signum();
        let after = world_from_space(w.view.pos, w.view.yaw).transform_point3(head);
        w.view.pos += before - after;
    }
}

/// Draws the frame for the headset: the level for each eye, and the UI on
/// its panel.
pub(crate) fn render(w: &mut World) {
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
    let Ok(state) = x.waiter.wait() else {
        return;
    };
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
    let anchor = world_from_space(w.view.pos, w.view.yaw);
    let size = eyes.size;
    for (eye, view) in views.iter().enumerate() {
        let camera = eye_camera(&w.camera, anchor, view);
        let target = &eyes.images[image].1[eye];
        let mut encoder = device.create_command_encoder(&Default::default());
        clear(&mut encoder, target, w.clear.0.to_linear());
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
        x.ui.get_or_insert_with(|| UiGpu::new(gpu, TARGET_FORMAT))
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
    let (centre, rotation) = panel_pose();
    let quad = xr::CompositionLayerQuad::new()
        .space(&x.space)
        .layer_flags(xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
        .eye_visibility(xr::EyeVisibility::BOTH)
        .sub_image(
            xr::SwapchainSubImage::new()
                .swapchain(&panel.handle)
                .image_rect(rect(panel.size)),
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
            width: PANEL_WIDTH,
            height: panel_height(panel.size),
        });
    if let Err(e) = x
        .stream
        .end(state.predicted_display_time, x.blend, &[&projection, &quad])
    {
        warn!("VR frame: {e}");
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

/// The level's coordinates of a point in the headset's space (metres, from
/// where the head began): the head's start is the camera at `pos` facing
/// its `yaw`.
fn world_from_space(pos: Vec3, yaw: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(1.0 / METRES_PER_UNIT),
        anchor_rotation(yaw),
        pos,
    )
}

/// The scene camera for an eye at `view`, the rest as the game's `base`.
fn eye_camera(base: &SceneCamera, anchor: Mat4, view: &xr::View) -> SceneCamera {
    let (position, rotation) = pose(view.pose);
    let world_from_eye = anchor * Mat4::from_rotation_translation(rotation, position);
    let view_proj = fov_projection(view.fov) * world_from_eye.inverse();
    let forward = world_from_eye.transform_vector3(Vec3::NEG_Z).normalize();
    let right = world_from_eye.transform_vector3(Vec3::X).normalize();
    // The sky: the horizon where the level's horizontal ahead meets the
    // view, and the panorama turned with the heading.
    let level = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::X);
    let clip = view_proj * level.extend(0.0);
    let horizon = if clip.w > 1e-4 {
        (1.0 - clip.y / clip.w) / 2.0
    } else if forward.y > 0.0 {
        2.0
    } else {
        -1.0
    };
    let yaw = (-forward.z).atan2(-forward.x);
    SceneCamera {
        view_proj,
        horizon,
        sky_column: crate::sky_left_column(yaw),
        right,
        roll: (-right.y).clamp(-1.0, 1.0).asin(),
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

/// The panel's centre and turn in the headset's space.
fn panel_pose() -> (Vec3, Quat) {
    (PANEL_CENTRE, Quat::from_rotation_x(-PANEL_TILT))
}

/// The panel's height in metres for an image of `size`.
fn panel_height(size: UVec2) -> f32 {
    PANEL_WIDTH * size.y as f32 / size.x.max(1) as f32
}

/// Where a ray from `origin` along `dir` meets the panel showing the screen
/// `panel` (its pixels' rectangle), as 0–1 across and down; none if it
/// misses.
fn panel_hit(origin: Vec3, dir: Vec3, panel: UVec4) -> Option<Vec2> {
    let (centre, rotation) = panel_pose();
    let normal = rotation * Vec3::Z;
    let facing = dir.dot(normal);
    if facing >= -1e-4 {
        return None;
    }
    let t = (centre - origin).dot(normal) / facing;
    if t <= 0.0 {
        return None;
    }
    let local = rotation.inverse() * (origin + dir * t - centre);
    let height = panel_height(UVec2::new(panel.z, panel.w));
    let uv = Vec2::new(local.x / PANEL_WIDTH + 0.5, 0.5 - local.y / height);
    (uv.cmpge(Vec2::ZERO).all() && uv.cmple(Vec2::ONE).all()).then_some(uv)
}
