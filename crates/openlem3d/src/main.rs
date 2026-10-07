//! openlem3d: level viewer (first milestone of the game).
//!
//! Usage: `openlem3d [--data DIR] [--level N] [--camera 1-4] [--screenshot FILE]`
//! Without `--level` the game starts at the level-select screen.
//!
//! Controls: W/S/A/D or arrows move, Q/E or right-drag turn, R/F rise/fall,
//! 1–4 preset cameras, P pause, [ / ] previous/next level, Esc level select,
//! M mute music.

mod briefing;
mod codes;
mod fade;
mod hud;
mod lemming_cam;
mod panel;
mod pointer;
mod practice;
mod menu;
mod minimap;
mod music;
mod title;
mod touch;
mod lemming_render;
mod level_mesh;
mod results;
mod scene_build;
mod options;
mod scene_render;
mod settings;
mod sfx;

use std::path::PathBuf;
use std::sync::Arc;

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_formats::blk::BlockSet;
use l3d_formats::level::{CameraPreset, Level};
use scene_render::{SceneCamera, SceneContent, SceneRenderPlugin};

/// Command-line options.
#[derive(Resource, Clone)]
pub struct Options {
    data: Option<PathBuf>,
    /// Level to start in directly; the level-select screen otherwise.
    level: Option<u32>,
    camera: Option<usize>,
    screenshot: Option<PathBuf>,
    /// Vertical field of view in degrees.
    fov_y: f32,
    /// Height of the horizon on screen, as a fraction from the top.
    horizon: f32,
    /// Initial window size in physical pixels.
    size: Option<(u32, u32)>,
    /// Seconds after loading before `--screenshot` captures.
    wait: f32,
    /// Hide all on-screen UI (for side-by-side comparisons with the original).
    no_hud: bool,
    /// Scripted skill assignments `(tick, lemming, skill id, turner side)`,
    /// from `--assign TICK:LEMMING:SKILL[:cw|acw]`.
    assign: Vec<(u64, usize, u8, Option<String>)>,
    /// Lemming the virtual-lemming camera follows from the start (`--follow N`).
    follow: Option<usize>,
    /// The screen to start on without `--level` (`--screen title|code|options|practice`).
    screen: menu::AppState,
    /// With `--level`, start on that level's briefing (`--briefing`).
    briefing: bool,
    /// Scripted presses `(seconds since start, key, seconds held)`, from
    /// `--press SECONDS:KEY[:HOLD]`.
    press: Vec<(f32, String, f32)>,
}

fn parse_args() -> Options {
    let mut o = Options {
        data: None,
        level: None,
        camera: None,
        screenshot: None,
        fov_y: DEFAULT_FOV_Y,
        horizon: DEFAULT_HORIZON,
        size: None,
        wait: 0.5,
        no_hud: false,
        assign: Vec::new(),
        follow: None,
        screen: menu::AppState::Title,
        briefing: false,
        press: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--data" => o.data = Some(val().into()),
            "--level" => o.level = Some(val().parse().expect("--level takes a number")),
            "--camera" => o.camera = Some(val().parse().expect("--camera takes 1-4")),
            "--screenshot" => o.screenshot = Some(val().into()),
            "--fov" => o.fov_y = val().parse().expect("--fov takes degrees"),
            "--horizon" => o.horizon = val().parse().expect("--horizon takes a fraction"),
            "--wait" => o.wait = val().parse().expect("--wait takes seconds"),
            "--no-hud" => o.no_hud = true,
            "--screen" => {
                o.screen = match val().as_str() {
                    "title" => menu::AppState::Title,
                    "code" => menu::AppState::Code,
                    "options" => menu::AppState::Options,
                    "practice" => menu::AppState::Practice,
                    "menu" => menu::AppState::Menu,
                    s => panic!("--screen takes title, code, options, practice or menu, not {s}"),
                }
            }
            "--briefing" => o.briefing = true,
            "--press" => {
                let v = val();
                let p: Vec<&str> = v.split(':').collect();
                let secs = |i: usize| p.get(i).map(|s| s.parse::<f32>().expect("--press takes SECONDS:KEY[:HOLD]"));
                o.press.push((secs(0).expect("--press seconds"), p.get(1).expect("--press key").to_string(), secs(2).unwrap_or(0.0)));
            }
            "--follow" => o.follow = Some(val().parse().expect("--follow takes a lemming number")),
            "--assign" => {
                let v = val();
                let p: Vec<&str> = v.split(':').collect();
                let num = |i: usize| p.get(i).and_then(|s| s.parse().ok()).expect("--assign takes TICK:LEMMING:SKILL[:cw|acw]");
                o.assign.push((num(0), num(1) as usize, num(2) as u8, p.get(3).map(|s| s.to_string())));
            }
            "--size" => {
                let v = val();
                let (w, h) = v.split_once('x').expect("--size takes WxH");
                o.size = Some((w.parse().expect("width"), h.parse().expect("height")));
            }
            _ => panic!("unknown argument {a}"),
        }
    }
    o
}

/// GPU backend selection. Vulkan on Windows-on-ARM (Adreno) loses the device
/// after a few seconds, so Windows defaults to DX12. `WGPU_BACKEND` overrides.
fn wgpu_settings() -> WgpuSettings {
    let mut s = WgpuSettings::default();
    if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
        s.backends = Some(Backends::DX12);
    }
    s
}

fn main() {
    let opts = parse_args();
    let dir = locate_data_dir(opts.data.as_deref());
    let data = GameData::open(&dir).unwrap_or_else(|e| {
        eprintln!("Cannot open the Lemmings 3D CD image in {}: {e}", dir.display());
        eprintln!("Put the .cue and .bin in ./gamedata, set OPENLEM3D_DATA, or pass --data DIR.");
        std::process::exit(1);
    });
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "openlem3d".into(),
                        resolution: match opts.size {
                            Some((w, h)) => bevy::window::WindowResolution::new(w, h).with_scale_factor_override(1.0),
                            None => default(),
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(wgpu_settings())),
                    ..default()
                }),
        )
        .add_plugins((SceneRenderPlugin, hud::HudPlugin, menu::MenuPlugin, music::MusicPlugin, touch::TouchPlugin, title::TitlePlugin, lemming_cam::LemmingCamPlugin, panel::PanelPlugin, briefing::BriefingPlugin, minimap::MinimapPlugin, results::ResultsPlugin, pointer::PointerPlugin, sfx::SfxPlugin, options::OptionsPlugin, practice::PracticePlugin))
        .add_plugins(fade::FadePlugin)
        .insert_resource(ClearColor(Color::srgb(0.35, 0.55, 0.85)))
        .insert_resource(opts.clone())
        .insert_resource(Data(data))
        .insert_resource(CurrentLevel { number: opts.level.unwrap_or(0), loaded: None })
        .insert_state(match opts.level {
            Some(_) if opts.briefing => menu::AppState::Briefing,
            Some(_) => menu::AppState::Playing,
            None => opts.screen,
        })
        .add_systems(Update, (screenshot_when_ready, hide_ui))
        .add_systems(PreUpdate, scripted_presses.after(bevy::input::InputSystems))
        .insert_resource(Game::default())
        .insert_resource(Time::<Fixed>::from_hz(l3d_sim::TICKS_PER_SECOND as f64))
        .init_resource::<PresetIndex>()
        .add_systems(FixedUpdate, step_simulation.run_if(in_state(menu::AppState::Playing)))
        .add_systems(OnEnter(menu::AppState::Briefing), |mut game: ResMut<Game>| game.replay = None)
        .add_systems(PostUpdate, update_lemming_sprites)
        .add_systems(Startup, spawn_camera)
        .add_systems(
            Update,
            (switch_level, load_level, refresh_scenery, camera_controls).chain().run_if(in_state(menu::AppState::Playing)),
        )
        .run();
}

#[derive(Resource)]
pub struct Data(pub GameData);

/// The level being shown; `loaded` lags `number` until the level is built.
#[derive(Resource)]
pub struct CurrentLevel {
    number: u32,
    loaded: Option<u32>,
}

/// The preset camera last chosen (0–3).
#[derive(Resource, Default)]
pub struct PresetIndex(pub usize);

/// The current level's preset cameras.
#[derive(Resource)]
struct LevelInfo {
    cameras: [CameraPreset; 4],
}

/// The player camera: free horizontal position and yaw; never pitches.
#[derive(Component)]
struct ViewCamera {
    pos: Vec3,
    /// Facing in radians; 0 faces −X, π/2 faces −Z (the original's rotation
    /// steps, see `docs/spec/level.md`).
    yaw: f32,
    /// Roll in radians about the view direction, turning the picture
    /// anticlockwise; 0 except in the lemming view.
    roll: f32,
}

impl ViewCamera {
    fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    fn set_preset(&mut self, p: &CameraPreset) {
        self.pos = Vec3::new(16.0 + p.x as f32 / 256.0, 8.0 + p.y as f32 / 256.0, 16.0 + p.z as f32 / 256.0);
        self.yaw = p.rotation as f32 * std::f32::consts::FRAC_PI_2;
    }
}

/// Whether the camera at `p` would be inside a block that stops it: any
/// non-empty cell within [`CAMERA_RADIUS`] whose present segments reach the
/// camera's height, unless the block is flagged passable for the camera.
fn camera_blocked(level: &Level, blocks: &BlockSet, p: Vec3) -> bool {
    use l3d_formats::level::{SIZE_X, SIZE_Y, SIZE_Z};
    let r = CAMERA_RADIUS;
    for dx in [-r, r] {
        for dy in [-r, r] {
            for dz in [-r, r] {
                let q = p + Vec3::new(dx, dy, dz);
                let c = q.floor();
                if c.x < 0.0 || c.y < 0.0 || c.z < 0.0 || c.x >= SIZE_X as f32 || c.y >= SIZE_Y as f32 || c.z >= SIZE_Z as f32 {
                    continue;
                }
                let b = level.block(c.x as usize, c.y as usize, c.z as usize);
                if b.is_empty() || blocks.defs.get(b.id as usize).is_some_and(|d| d.flags & l3d_formats::blk::flags::NON_SOLID_CAMERA != 0) {
                    continue;
                }
                let seg = ((q.y - c.y) * 4.0) as u8;
                if b.segments & (1 << seg.min(3)) != 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// How close the camera may come to a block (grid units).
const CAMERA_RADIUS: f32 = 0.2;

/// Vertical field of view of the scene camera.
const DEFAULT_FOV_Y: f32 = 51.0;

/// Default horizon height (fraction of the screen from the top). The original
/// never pitches its camera but draws the horizon well above the middle of
/// the screen, i.e. it uses an off-centre projection.
const DEFAULT_HORIZON: f32 = 0.3125;

/// Perspective projection whose horizon (the camera's eye level) lies at
/// `horizon` (fraction from the top of the screen) instead of the centre.
fn projection(fov_y_deg: f32, aspect: f32, horizon: f32) -> Mat4 {
    // Adding `shift · w_clip` (w_clip = −z_view) to clip-space y moves every
    // NDC y by `shift`; the horizon (y_view = 0, z_view → −∞) lands at `shift`.
    let shift = 1.0 - 2.0 * horizon;
    // The far plane reaches the edge of the sea plane (see scene_build).
    let mut p = Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, 0.05, 8000.0);
    p.z_axis.y -= shift;
    p
}

/// Clip-space rotation by `roll` (anticlockwise on screen) about the
/// horizon's centre, the point the off-centre projection puts eye level at;
/// `aspect` keeps the rotation true on a non-square screen.
fn roll_about_horizon(roll: f32, aspect: f32, horizon: f32) -> Mat4 {
    if roll == 0.0 {
        return Mat4::IDENTITY;
    }
    let shift = 1.0 - 2.0 * horizon;
    // Clip-space translations scale with w, so they move NDC points as wanted.
    Mat4::from_translation(Vec3::Y * shift)
        * Mat4::from_scale(Vec3::new(1.0 / aspect, 1.0, 1.0))
        * Mat4::from_rotation_z(roll)
        * Mat4::from_scale(Vec3::new(aspect, 1.0, 1.0))
        * Mat4::from_translation(Vec3::Y * -shift)
}

/// Sky texel column at the left edge of a 640-wide screen for a camera
/// facing `yaw`. The 1024-texel panorama spans one full turn and each texel
/// covers two screen pixels; facing +Z (yaw 3π/2) puts column 928 at the
/// screen centre (`docs/spec/camera.md`).
fn sky_left_column(yaw: f32) -> f32 {
    const CENTRE_AT_POS_Z: f32 = 928.0;
    let centre = CENTRE_AT_POS_Z + (yaw - 3.0 * std::f32::consts::FRAC_PI_2) * 1024.0 / std::f32::consts::TAU;
    (centre - 160.0).rem_euclid(1024.0)
}

fn spawn_camera(mut commands: Commands) {
    // A 2D camera drives the frame; the scene renderer draws the 3D level
    // into its target before sprites and UI.
    commands.spawn((Camera2d, Tonemapping::None, Msaa::Off, ViewCamera { pos: Vec3::new(16.0, 8.0, 16.0), yaw: 0.0, roll: 0.0 }));
}

/// Whether the level about to be played is starting afresh, rather than
/// resuming after the options screen.
pub fn fresh_level(current: Res<CurrentLevel>) -> bool {
    current.loaded.is_none()
}

fn switch_level(keys: Res<ButtonInput<KeyCode>>, mut current: ResMut<CurrentLevel>) {
    if keys.just_pressed(KeyCode::BracketRight) {
        current.number = (current.number + 1) % 100;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        current.number = (current.number + 99) % 100;
    }
}

/// The level being played.
#[derive(Resource, Default)]
pub struct Game {
    pub sim: Option<l3d_sim::Simulation>,
    pub paused: bool,
    /// Fast-forward: several simulation ticks per tick.
    pub fast_forward: bool,
    pub save_requirement: u32,
    /// The level's block grid as currently shaped (destructible skills change
    /// it), its block dictionary, and the scene layer showing it.
    terrain: Option<(Level, BlockSet, usize)>,
    /// The level restarted as a replay of an earlier attempt, until the
    /// player takes over.
    pub replay: Option<Replay>,
    /// The exits' doors as drawn.
    doors: Vec<lemming_render::Door>,
}

/// An earlier attempt's commands, fed back into a restarted level.
pub struct Replay {
    log: Vec<(u64, l3d_sim::Command)>,
    /// Commands applied so far.
    next: usize,
    /// The length of the simulation's own log after the last command
    /// applied; anything beyond it is the player acting.
    logged: usize,
    /// For a demo rather than the player's own attempt: the screen any input,
    /// or the level ending, returns to.
    pub demo: Option<menu::AppState>,
}

impl Replay {
    pub fn new(log: Vec<(u64, l3d_sim::Command)>) -> Self {
        Replay { log, next: 0, logged: 0, demo: None }
    }

    pub fn demo(log: Vec<(u64, l3d_sim::Command)>, back_to: menu::AppState) -> Self {
        Replay { demo: Some(back_to), ..Replay::new(log) }
    }

    /// Applies the commands due before the simulation's next step. Returns
    /// false once the player has acted on their own, which ends the replay.
    fn feed(&mut self, sim: &mut l3d_sim::Simulation) -> bool {
        if sim.log.len() != self.logged {
            return false;
        }
        while let Some(&(_, c)) = self.log.get(self.next).filter(|(t, _)| *t <= sim.tick) {
            sim.apply(c);
            self.next += 1;
        }
        self.logged = sim.log.len();
        true
    }
}

/// Simulation ticks per tick while fast-forwarding (the original's speed-up
/// is unmeasured).
const FAST_FORWARD_TICKS: u32 = 3;

fn step_simulation(keys: Res<ButtonInput<KeyCode>>, options: Res<Options>, mut game: ResMut<Game>, mut scene: ResMut<SceneContent>) {
    if keys.just_pressed(KeyCode::KeyP) {
        game.paused = !game.paused;
    }
    let Game { sim: Some(sim), paused: false, terrain, fast_forward, replay, doors, .. } = &mut *game else { return };
    for _ in 0..if *fast_forward { FAST_FORWARD_TICKS } else { 1 } {
        if replay.as_mut().is_some_and(|r| !r.feed(sim)) {
            *replay = None;
        }
        sim.step();
        lemming_render::step_doors(doors, sim);
        for (tick, i, skill, side) in &options.assign {
            if *tick == sim.tick
                && replay.is_none()
                && let (Some(skill), Some(dir)) = (l3d_sim::Skill::from_id(*skill), sim.lemmings.get(*i).map(|l| l.dir))
            {
                match side.as_deref() {
                    Some("cw") => sim.assign_turner(*i, dir.clockwise()),
                    Some("acw") => sim.assign_turner(*i, dir.anticlockwise()),
                    _ => sim.assign(*i, skill),
                };
            }
        }
    }
    let changes = sim.world.take_changes();
    let bricks_changed = sim.world.take_bricks_changed();
    if changes.is_empty() && !bricks_changed {
        return;
    }
    let (Some((level, blocks, layer)), Some(current)) = (terrain, &scene.data) else { return };
    for ([x, y, z], cell) in changes {
        level.set_block(x, y, z, cell);
    }
    let rebuilt = scene_build::rebuild_blocks(current, *layer, level, blocks, &sim.world.bricks);
    scene.version += 1;
    scene.data = Some(Arc::new(rebuilt));
}

fn update_lemming_sprites(
    game: Res<Game>,
    cams: Query<&ViewCamera>,
    lemming_cam: Res<lemming_cam::LemmingCam>,
    mut sprites: ResMut<scene_render::SceneSprites>,
) {
    let Some(sim) = &game.sim else { return };
    let yaw = cams.iter().next().map_or(0.0, |c| c.yaw);
    *sprites = lemming_render::build(sim, scene_build::ATLAS_ROWS, yaw, &game.doors, lemming_cam.following());
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn load_level(
    mut commands: Commands,
    mut current: ResMut<CurrentLevel>,
    mut data: ResMut<Data>,
    opts: Res<Options>,
    mut scene: ResMut<SceneContent>,
    mut game: ResMut<Game>,
    (mut sources, music, muted, settings): (ResMut<Assets<bevy::audio::AudioSource>>, music::MusicQuery, Res<music::MusicMuted>, Res<settings::Settings>),
    mut cams: Query<&mut ViewCamera>,
    mut windows: Query<&mut Window>,
) {
    if current.loaded == Some(current.number) {
        return;
    }
    let n = current.number;
    current.loaded = Some(n);
    let scene_build::BuiltLevel { level, blocks, scene: content, mesh, block_layer } = match scene_build::build(&mut data.0, n, scene_build::Show { land: settings.land, sea: settings.sea, sky: settings.sky }) {
        Ok(v) => v,
        Err(e) => {
            error!("level {n}: {e}");
            return;
        }
    };
    if !mesh.unsupported_shapes.is_empty() {
        warn!("level {n}: unsupported shapes (shape, cells): {:?}", mesh.unsupported_shapes);
    }
    info!(
        "level {n:03} {:?}: texture set {}, {} opaque + {} cutout triangles",
        level.title,
        level.texture_set,
        mesh.opaque.indices.len() / 3,
        mesh.cutout.indices.len() / 3
    );
    for mut w in &mut windows {
        w.title = format!("openlem3d - LEVEL.{n:03} {}", level.title);
    }
    scene.version += 1;
    scene.data = Some(Arc::new(content));
    let preset = opts.camera.filter(|c| (1..=4).contains(c)).unwrap_or(1) - 1;
    commands.insert_resource(PresetIndex(preset));
    game.fast_forward = false;
    game.paused = false;
    for mut cam in &mut cams {
        cam.set_preset(&level.cameras[preset]);
    }
    commands.insert_resource(LevelInfo { cameras: level.cameras });
    game.sim = Some(l3d_sim::Simulation::new(&level, &blocks));
    game.doors = lemming_render::doors(&level, &blocks);
    game.save_requirement = level.save_requirement as u32;
    let track = music::track_for(level.theme, level.music);
    music::play_track(&mut commands, &mut sources, &music::current(&music), &data.0.disc, track, music::volume(&muted, &settings));
    game.terrain = Some((level, blocks, block_layer));
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn camera_controls(
    opts: Res<Options>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    info: Option<Res<LevelInfo>>,
    windows: Query<&Window>,
    mut scene_cam: ResMut<SceneCamera>,
    lemming_cam: Res<lemming_cam::LemmingCam>,
    mut preset: ResMut<PresetIndex>,
    settings: Res<settings::Settings>,
    game: Res<Game>,
    mut last_free: Local<Option<Vec3>>,
    mut q: Query<&mut ViewCamera>,
    (scroll, ui, mut held_drag): (Res<AccumulatedMouseScroll>, Query<&Interaction>, Local<Option<f32>>),
) {
    // Holding the camera button: a drag turns the view; held still over the
    // view, the camera moves as the pointer's arrow shows (see `pointer`).
    let turn_button = settings.turn_button();
    if mouse.just_pressed(turn_button) {
        *held_drag = (!ui.iter().any(|i| *i != Interaction::None)).then_some(0.0);
    }
    if !mouse.pressed(turn_button) {
        *held_drag = None;
    }
    if let Some(d) = held_drag.as_mut() {
        *d += motion.delta.length();
    }
    let dragging = held_drag.is_some_and(|d| d > DRAG_SLOP);
    let region = held_drag
        .filter(|_| !dragging)
        .and(windows.iter().next().and_then(|w| Some((w.cursor_position()?, Vec2::new(w.width(), w.height())))))
        .map(|(p, size)| pointer_region(p, size));
    // Riding along with a lemming: no manual movement.
    let dt = if lemming_cam.following().is_some() { 0.0 } else { time.delta_secs() };
    for mut cam in &mut q {
        let presets = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
        if let Some(info) = &info {
            for (i, k) in presets.iter().enumerate() {
                if keys.just_pressed(*k) {
                    cam.set_preset(&info.cameras[i]);
                    preset.0 = i;
                }
            }
        }
        let held = |a: KeyCode, b: KeyCode| (keys.pressed(a) || keys.pressed(b)) as i32 as f32;
        let turn = held(KeyCode::KeyE, KeyCode::KeyE) - held(KeyCode::KeyQ, KeyCode::KeyQ);
        cam.yaw += turn * 1.8 * dt;
        if dragging && dt > 0.0 {
            cam.yaw += motion.delta.x * 0.005;
        }
        // Region moves: forward, sideways and turning, from the 3×3 grid.
        let (ahead, side, spin) = region.map_or((0.0, 0.0, 0.0), |[c, r]| match (c, r) {
            (1, 0) => (1.0, 0.0, 0.0),
            (1, 2) => (-1.0, 0.0, 0.0),
            (0, 2) => (0.0, -1.0, 0.0),
            (2, 2) => (0.0, 1.0, 0.0),
            (0, 1) => (0.0, 0.0, -1.0),
            (2, 1) => (0.0, 0.0, 1.0),
            (0, 0) => (1.0, 0.0, -1.0),
            (2, 0) => (1.0, 0.0, 1.0),
            _ => (0.0, 0.0, 0.0),
        });
        cam.yaw += spin * 1.8 * dt;
        let fwd = cam.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let speed = if keys.pressed(KeyCode::ShiftLeft) { 16.0 } else { 6.0 } * settings.camera_factor();
        let mv = fwd * (held(KeyCode::KeyW, KeyCode::ArrowUp) - held(KeyCode::KeyS, KeyCode::ArrowDown))
            + right * (held(KeyCode::KeyD, KeyCode::ArrowRight) - held(KeyCode::KeyA, KeyCode::ArrowLeft))
            + Vec3::Y * (held(KeyCode::KeyR, KeyCode::PageUp) - held(KeyCode::KeyF, KeyCode::PageDown))
            + fwd * ahead
            + right * side;
        cam.pos += mv * speed * dt;
        // The wheel raises and lowers the camera.
        if dt > 0.0 {
            let notches = match scroll.unit {
                MouseScrollUnit::Line => scroll.delta.y,
                MouseScrollUnit::Pixel => scroll.delta.y / 50.0,
            };
            cam.pos.y += notches * WHEEL_STEP;
        }
        // The camera can't enter blocks (unless they are marked passable for
        // it); it slides along them. Preset jumps and the lemming view are
        // exempt.
        if let Some((level, blocks, _)) = &game.terrain {
            let jumped = keys.any_just_pressed(presets) || lemming_cam.following().is_some();
            if let Some(free) = *last_free
                && !jumped
                && camera_blocked(level, blocks, cam.pos)
            {
                let mut p = free;
                for axis in 0..3 {
                    let mut q = p;
                    q[axis] = cam.pos[axis];
                    if !camera_blocked(level, blocks, q) {
                        p = q;
                    }
                }
                cam.pos = p;
            }
            if !camera_blocked(level, blocks, cam.pos) {
                *last_free = Some(cam.pos);
            }
        }
        let aspect = windows.iter().next().map_or(16.0 / 9.0, |w| w.width() / w.height().max(1.0));
        let view = Mat4::look_to_rh(cam.pos, cam.forward(), Vec3::Y);
        scene_cam.view_proj = roll_about_horizon(cam.roll, aspect, opts.horizon) * projection(opts.fov_y, aspect, opts.horizon) * view;
        scene_cam.roll = cam.roll;
        scene_cam.horizon = opts.horizon;
        scene_cam.time = time.elapsed_secs();
        scene_cam.sky_column = sky_left_column(cam.yaw);
        scene_cam.right = right;
    }
}

/// With `--screenshot`, saves one frame `--wait` seconds after the level has
/// loaded, then exits.
fn screenshot_when_ready(
    mut commands: Commands,
    opts: Res<Options>,
    current: Res<CurrentLevel>,
    state: Res<State<menu::AppState>>,
    time: Res<Time>,
    mut elapsed: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &opts.screenshot else { return };
    if current.loaded.is_none() && !matches!(state.get(), menu::AppState::Menu | menu::AppState::Title | menu::AppState::Code | menu::AppState::Briefing | menu::AppState::Options | menu::AppState::Practice) {
        return;
    }
    let before = elapsed.unwrap_or(-time.delta_secs());
    let now = before + time.delta_secs();
    *elapsed = Some(now);
    let shot_at = opts.wait.max(0.5);
    if before < shot_at && now >= shot_at {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
    }
    if now >= shot_at + 0.5 {
        exit.write(AppExit::Success);
    }
}

/// With `--no-hud`, hides every top-level UI node.
fn hide_ui(opts: Res<Options>, mut roots: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>) {
    if !opts.no_hud {
        return;
    }
    for mut v in &mut roots {
        *v = Visibility::Hidden;
    }
}

/// Rebuilds the scenery when the land, sea or sky setting changes during a
/// level (on the options screen), keeping the terrain as the lemmings left it.
fn refresh_scenery(
    settings: Res<settings::Settings>,
    current: Res<CurrentLevel>,
    mut data: ResMut<Data>,
    mut scene: ResMut<SceneContent>,
    mut game: ResMut<Game>,
    mut shown: Local<Option<(u32, scene_build::Show)>>,
) {
    let Some(n) = current.loaded else { return };
    let show = scene_build::Show { land: settings.land, sea: settings.sea, sky: settings.sky };
    let was = shown.replace((n, show));
    if was.is_none_or(|(m, s)| m != n || s == show) {
        return;
    }
    let built = match scene_build::build(&mut data.0, n, show) {
        Ok(b) => b,
        Err(e) => {
            error!("level {n}: {e}");
            return;
        }
    };
    let Game { sim, terrain, .. } = &mut *game;
    let content = match (terrain.as_mut(), sim) {
        (Some((level, blocks, layer)), Some(sim)) => {
            *layer = built.block_layer;
            scene_build::rebuild_blocks(&built.scene, built.block_layer, level, blocks, &sim.world.bricks)
        }
        _ => built.scene,
    };
    scene.version += 1;
    scene.data = Some(Arc::new(content));
}

/// Presses the keys given with `--press SECONDS:KEY[:HOLD]` at their times,
/// for HOLD seconds or one frame: key names as in Bevy's `KeyCode`
/// (`Escape`, `F12`, `KeyW`, `Digit1`, …), or `Click` for the left mouse button.
fn scripted_presses(
    opts: Res<Options>,
    time: Res<Time<Real>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut done: Local<usize>,
    mut held: Local<Vec<(String, f32)>>,
) {
    let now = time.elapsed_secs();
    held.retain(|(k, until)| {
        if *until > now {
            return true;
        }
        match key_code(k) {
            Some(code) => keys.release(code),
            None => mouse.release(MouseButton::Left),
        }
        false
    });
    while let Some((t, k, hold)) = opts.press.get(*done).filter(|(t, ..)| *t <= now) {
        info!("scripted press {k} at {t} s");
        match key_code(k) {
            Some(code) => keys.press(code),
            None if k == "Click" => mouse.press(MouseButton::Left),
            None => warn!("--press: unknown key {k}"),
        }
        held.push((k.clone(), t + hold));
        *done += 1;
    }
}

fn key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Escape" => KeyCode::Escape,
        "Enter" => KeyCode::Enter,
        "Space" => KeyCode::Space,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F12" => KeyCode::F12,
        "KeyP" => KeyCode::KeyP,
        "KeyV" => KeyCode::KeyV,
        "KeyI" => KeyCode::KeyI,
        "KeyM" => KeyCode::KeyM,
        "KeyW" => KeyCode::KeyW,
        "KeyA" => KeyCode::KeyA,
        "KeyS" => KeyCode::KeyS,
        "KeyD" => KeyCode::KeyD,
        "KeyQ" => KeyCode::KeyQ,
        "KeyE" => KeyCode::KeyE,
        "KeyR" => KeyCode::KeyR,
        "KeyF" => KeyCode::KeyF,
        "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2,
        "Digit3" => KeyCode::Digit3,
        "Digit4" => KeyCode::Digit4,
        _ => return None,
    })
}

/// Pointer travel (logical pixels) after which holding the camera button is
/// a drag that turns the view rather than a hold that moves it.
const DRAG_SLOP: f32 = 6.0;
/// Camera rise per wheel notch (grid units).
const WHEEL_STEP: f32 = 0.25;

/// The cell `[column, row]` of the 3×3 grid over the view that `p` is in.
pub fn pointer_region(p: Vec2, size: Vec2) -> [usize; 2] {
    [((p.x / size.x * 3.0) as usize).min(2), ((p.y / size.y * 3.0) as usize).min(2)]
}
