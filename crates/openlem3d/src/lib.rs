//! openlem3d: the game, from the title screen and menus through briefings,
//! play and results, drawn from the user's original CD data.
//!
//! Usage: `openlem3d [--data DIR] [--level N] [OPTIONS]`; the further
//! options (`--camera`, `--screen`, `--screenshot`, `--press`, …) are those
//! of `parse_args`. Without `--level` the game starts at the title screen.

mod briefing;
mod codes;
mod confirm;
mod fade;
mod glow;
mod gpu;
mod hdr_blink;
mod hud;
mod key_buttons;
mod lemming_cam;
mod lemming_render;
mod level_mesh;
mod light;
mod menu;
mod minimap;
mod music;
mod options;
mod panel;
mod pixels;
mod platform;
mod pointer;
mod practice;
mod profile;
mod results;
mod scene_build;
mod scene_render;
mod screen;
mod settings;
mod sfx;
mod style;
mod texture;
mod title;
mod touch;
mod trackpad;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use l3d_engine::input::MouseScrollUnit;
use l3d_engine::prelude::*;
use l3d_formats::blk::BlockSet;
use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_formats::level::{CameraPreset, Level};
use scene_render::{SceneCamera, SceneContent, SceneSprites};
use ui::Ui;

/// A simulation length or coordinate in sub-units ([`l3d_sim::SUB`] to the
/// unit), in units.
pub(crate) fn units(sub: i32) -> f32 {
    sub as f32 / l3d_sim::SUB as f32
}

/// Command-line options.
#[derive(Clone)]
pub struct Options {
    data: Option<PathBuf>,
    /// Level to start in directly; the level-select screen otherwise.
    level: Option<u32>,
    camera: Option<usize>,
    /// A camera placed by hand, `X,Y,Z,YAW` (units and radians), for testing.
    look: Option<(Vec3, f32)>,
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
    assign: Vec<l3d_sim::demos::Step>,
    /// Lemming the virtual-lemming camera follows from the start (`--follow N`).
    follow: Option<usize>,
    /// Play the level's stored solution as a replay of player input, so it
    /// ends on the results screen like a real attempt (`--solve`).
    solve: bool,
    /// Run game time as fast as the machine allows (`--turbo`), for scripted
    /// runs: `--wait` and `--press` count game time, so they mean the same.
    turbo: bool,
    /// The screen to start on without `--level` (`--screen title|code|options|practice`).
    screen: menu::AppState,
    /// With `--level`, start on that level's briefing (`--briefing`).
    briefing: bool,
    /// Scripted presses `(seconds since start, key, seconds held)`, from
    /// `--press SECONDS:KEY[:HOLD]`.
    press: Vec<(f32, String, f32)>,
    /// With `--screenshot`, also save a frame each simulation tick into a
    /// directory from some seconds in (`--record DIR:FROM`), as `TICK.png`.
    record: Option<(PathBuf, f32)>,
    /// Draw glowing things plainly (`--no-glow`), for comparison.
    no_glow: bool,
    /// Log where each frame's time goes, once a second (`--profile`).
    profile: bool,
    /// The graphics APIs to try (`--backend vulkan`, `gl`, `metal`, `dx12`),
    /// for drivers that fail with the usual one.
    backend: Option<wgpu::Backends>,
    /// Show glowing things brighter than white where the screen can
    /// (`--hdr`).
    hdr: bool,
    /// HDR and clipped at white by turns, labelled (`--hdr-blink`).
    hdr_blink: bool,
}

fn parse_args() -> Options {
    let mut o = Options {
        data: None,
        level: None,
        camera: None,
        look: None,
        screenshot: None,
        fov_y: DEFAULT_FOV_Y,
        horizon: DEFAULT_HORIZON,
        size: None,
        wait: 0.5,
        no_hud: false,
        assign: Vec::new(),
        follow: None,
        solve: false,
        turbo: false,
        screen: menu::AppState::Title,
        briefing: false,
        press: Vec::new(),
        record: None,
        no_glow: false,
        profile: false,
        backend: None,
        hdr: false,
        hdr_blink: false,
    };
    let mut args = platform::args().into_iter();
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--data" => o.data = Some(val().into()),
            "--level" => o.level = Some(val().parse().expect("--level takes a number")),
            "--camera" => o.camera = Some(val().parse().expect("--camera takes 1-4")),
            "--look" => {
                let v: Vec<f32> = val()
                    .split(',')
                    .map(|s| s.parse().expect("--look takes X,Y,Z,YAW"))
                    .collect();
                o.look = Some((Vec3::new(v[0], v[1], v[2]), v[3]));
            }
            "--screenshot" => o.screenshot = Some(val().into()),
            "--fov" => o.fov_y = val().parse().expect("--fov takes degrees"),
            "--horizon" => o.horizon = val().parse().expect("--horizon takes a fraction"),
            "--wait" => o.wait = val().parse().expect("--wait takes seconds"),
            "--record" => {
                let v = val();
                let (dir, from) = v.rsplit_once(':').expect("--record takes DIR:FROM");
                o.record = Some((
                    dir.into(),
                    from.parse().expect("--record FROM takes seconds"),
                ));
            }
            "--no-hud" => o.no_hud = true,
            "--no-glow" => o.no_glow = true,
            "--profile" => o.profile = true,
            "--hdr" => o.hdr = true,
            "--hdr-blink" => (o.hdr, o.hdr_blink) = (true, true),
            "--backend" => o.backend = Some(wgpu::Backends::from_comma_list(&val())),
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
                let secs = |i: usize| {
                    p.get(i)
                        .map(|s| s.parse::<f32>().expect("--press takes SECONDS:KEY[:HOLD]"))
                };
                o.press.push((
                    secs(0).expect("--press seconds"),
                    p.get(1).expect("--press key").to_string(),
                    secs(2).unwrap_or(0.0),
                ));
            }
            "--solve" => o.solve = true,
            "--turbo" => o.turbo = true,
            "--follow" => o.follow = Some(val().parse().expect("--follow takes a lemming number")),
            "--assign" => o.assign.push(
                l3d_sim::demos::parse_step(&val())
                    .expect("--assign takes TICK:LEMMING:SKILL[:cw|acw]"),
            ),
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

/// Android's entry point: keeps the app the system started and runs the
/// game.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: l3d_engine::android::android_activity::AndroidApp) {
    let _ = l3d_engine::android::ANDROID_APP.set(app);
    main();
}

pub fn main() {
    let opts = parse_args();
    // Before the window shows, so the game doesn't lay itself out around
    // the system bars first and then again without them.
    platform::full_screen();
    let dir = locate_data_dir(opts.data.as_deref().or(platform::data_dir().as_deref()));
    let data = GameData::open(&dir).unwrap_or_else(|e| {
        eprintln!(
            "Cannot open the Lemmings 3D CD image in {}: {e}",
            dir.display()
        );
        eprintln!("{}", platform::DATA_HINT);
        std::process::exit(1);
    });
    let window = Window {
        title: "openlem3d".into(),
        present_mode: if opts.turbo {
            l3d_engine::window::PresentMode::AutoNoVsync
        } else {
            default()
        },
        resolution: match opts.size {
            Some((w, h)) => {
                l3d_engine::window::WindowResolution::new(w, h).with_scale_factor_override(1.0)
            }
            None => default(),
        },
        ..default()
    };
    let state = match opts.level {
        Some(_) if opts.briefing => menu::AppState::Briefing,
        Some(_) => menu::AppState::Playing,
        None => opts.screen,
    };
    l3d_engine::run(World::new(Engine::new(window), opts, Data(data), state));
}

/// Everything the game keeps: the engine's side (window, input, clocks,
/// pictures, sound) and each part's own state.
pub struct World {
    e: Engine,
    opts: Options,
    data: Data,
    /// The screen being shown, and the one asked for (taken up between the
    /// input and the fixed steps of the next frame).
    state: menu::AppState,
    next_state: Option<menu::AppState>,
    started: bool,
    settings: settings::Settings,
    current: CurrentLevel,
    game: Game,
    preset: PresetIndex,
    info: Option<LevelInfo>,
    view: ViewCamera,
    camera_controls: CameraControls,
    scene: SceneContent,
    camera: SceneCamera,
    sprites: SceneSprites,
    scenery_shown: Option<(u32, scene_build::Show)>,
    clear: gpu::ClearColor,
    screenshots: gpu::Screenshots,
    renderer: gpu::Renderer,
    profile: profile::Profile,
    shot: ShotTimer,
    scripted: Scripted,
    screen: screen::Screen,
    insets: Option<(f32, [u32; 4])>,
    pixelation: pixels::Pixelation,
    ui: Ui,
    art: Option<title::Art>,
    title: title::Title,
    screen_ui: Option<title::ScreenUi>,
    rating: menu::MenuRating,
    titles: Option<menu::Titles>,
    list: menu::LevelList,
    briefing: briefing::Briefing,
    preview: briefing::Preview,
    results: results::Results,
    options: options::OptionsScreen,
    options_from: options::OptionsFrom,
    practice: practice::Practice,
    panel: panel::Panel,
    minimap: Option<minimap::Minimap>,
    minimap_view: minimap::MinimapView,
    selected: hud::SelectedSkill,
    pending: hud::PendingTurner,
    ride_press: hud::RidePress,
    highlight: hud::Highlight,
    arrows: hud::TurnerArrows,
    lemming_cam: lemming_cam::LemmingCam,
    follow: lemming_cam::Follow,
    confirm: confirm::Confirm,
    fade: fade::Fade,
    style: style::Style,
    hdr_blink: hdr_blink::HdrBlink,
    key_buttons: key_buttons::KeyButtons,
    muted: music::MusicMuted,
    music: music::LevelMusic,
    sfx: sfx::Sounds,
    cells: Option<pointer::Cells>,
    touch_starts: touch::TouchStarts,
    touch_twist: touch::Twist,
    trackpad: trackpad::Trackpad,
}

impl World {
    /// Backspace held with rewinding on: the level runs back.
    fn backwards(&self) -> bool {
        self.settings.rewind && self.e.input.keys.pressed(KeyCode::Backspace)
    }

    fn new(mut e: Engine, opts: Options, data: Data, state: menu::AppState) -> Self {
        e.time.fixed.set_hz(l3d_sim::TICKS_PER_SECOND as f64);
        let profile = profile::Profile::new(opts.profile);
        let settings = settings::Settings::load();
        let rating = settings.rating;
        World {
            e,
            current: CurrentLevel {
                number: opts.level.unwrap_or(0),
                loaded: None,
            },
            opts,
            data,
            state,
            next_state: None,
            started: false,
            settings,
            game: Game::default(),
            preset: PresetIndex::default(),
            info: None,
            view: ViewCamera::default(),
            camera_controls: CameraControls::default(),
            scene: SceneContent::default(),
            camera: SceneCamera::default(),
            sprites: SceneSprites::default(),
            scenery_shown: None,
            clear: gpu::ClearColor(Color::srgb(0.35, 0.55, 0.85)),
            screenshots: gpu::Screenshots::default(),
            renderer: gpu::Renderer::default(),
            profile,
            shot: ShotTimer::default(),
            scripted: Scripted::default(),
            screen: screen::Screen::default(),
            insets: None,
            pixelation: pixels::Pixelation::default(),
            ui: Ui::default(),
            art: None,
            title: title::Title::default(),
            screen_ui: None,
            rating: menu::MenuRating(rating),
            titles: None,
            list: menu::LevelList::default(),
            briefing: briefing::Briefing::default(),
            preview: briefing::Preview::default(),
            results: results::Results::default(),
            options: options::OptionsScreen::default(),
            options_from: options::OptionsFrom::default(),
            practice: practice::Practice::default(),
            panel: panel::Panel::default(),
            minimap: None,
            minimap_view: minimap::MinimapView::default(),
            selected: hud::SelectedSkill::default(),
            pending: hud::PendingTurner::default(),
            ride_press: hud::RidePress::default(),
            highlight: hud::Highlight::default(),
            arrows: hud::TurnerArrows::default(),
            lemming_cam: lemming_cam::LemmingCam::default(),
            follow: lemming_cam::Follow::default(),
            confirm: confirm::Confirm::default(),
            fade: fade::Fade::default(),
            style: style::Style::default(),
            hdr_blink: hdr_blink::HdrBlink::default(),
            key_buttons: key_buttons::KeyButtons::default(),
            muted: music::MusicMuted::default(),
            music: music::LevelMusic::default(),
            sfx: sfx::Sounds::default(),
            cells: None,
            touch_starts: touch::TouchStarts::default(),
            touch_twist: touch::Twist::default(),
            trackpad: trackpad::Trackpad::default(),
        }
    }

    /// Once, before the first frame.
    fn startup(&mut self) {
        title::load_art(self);
        menu::read_titles(self);
        menu::load_labels(self);
        options::load_lights(self);
        options::apply_window(self);
        panel::load_art(self);
        panel::spawn_panel(self);
        hud::spawn_turner_marker(self);
        pointer::load(self);
        practice::load(self);
        fade::spawn(self);
        trackpad::start();
        turbo(self);
    }

    /// Leaving screen `s`.
    fn exit(&mut self, s: menu::AppState) {
        use menu::AppState::*;
        match s {
            Title | Options | Practice | Menu => title::exit_screen(self),
            Code => {
                title::keyboard(self, false);
                title::exit_screen(self);
            }
            Briefing => briefing::exit(self),
            Results => results::exit(self),
            Playing => {
                confirm::close(self);
                lemming_cam::exit_level(self);
                style::close_notice(self);
            }
        }
    }

    /// Entering screen `s`.
    fn enter(&mut self, s: menu::AppState) {
        use menu::AppState::*;
        music::enter(self, s);
        match s {
            Code => title::keyboard(self, true),
            Briefing => {
                self.game.replay = None;
                hud::enter_briefing(self);
            }
            Playing => sfx::lets_go(self),
            _ => {}
        }
    }

    /// Changes screen if one was asked for (even to the same one, which is
    /// left and entered again).
    fn change_state(&mut self) {
        let Some(entered) = self.next_state.take() else {
            return;
        };
        let exited = std::mem::replace(&mut self.state, entered);
        self.exit(exited);
        fade::transition(self, exited, entered);
        self.enter(entered);
    }

    /// The systems run each frame for the screen shown, in order.
    fn update(&mut self) {
        use menu::AppState::*;
        screenshot_when_ready(self);
        hide_ui(self);
        trackpad::watch(self);
        match self.state {
            Title => {
                title::attract(self);
                title::update_title(self);
                style::toggle_on_title(self);
                style::title_label(self);
                sfx::watch_rating(self);
                keep_rating(self);
            }
            Code => title::update_code(self),
            Options => {
                options::spawn_options(self);
                options::options_input(self);
                options::show_lights(self);
            }
            Practice => {
                practice::spawn_grid(self);
                practice::hover(self);
                practice::pick(self);
            }
            Menu => {
                menu::ensure_menu(self);
                menu::menu_input(self);
                menu::menu_to_title(self);
            }
            Briefing => {
                briefing::spawn_briefing(self);
                briefing::briefing_input(self);
            }
            Results => {
                results::spawn_results(self);
                results::results_input(self);
            }
            Playing => {
                briefing::end_demo(self);
                hud::skill_keys(self);
                hud::nuke_key(self);
                hud::assign_on_pointer(self);
                hud::take_over(self);
                lemming_cam::toggle_keys(self);
                menu::back_to_menu(self);
                confirm::show(self);
                style::toggle_in_level(self);
                options::open_in_level(self);
                results::level_over(self);
                sfx::sim_events(self);
                sfx::watch_play(self);
                pause_keys(self);
                load_level(self);
                refresh_scenery(self);
                briefing::preview(self);
                lemming_cam::follow(self);
                panel::before_camera(self);
                touch::touch_camera(self);
                trackpad::trackpad_camera(self);
                trackpad::gesture_camera(self);
                camera_controls(self);
                minimap::build_base(self);
                minimap::draw(self);
                minimap::click(self);
                panel::update(self);
            }
        }
        if matches!(self.state, Title | Code | Options | Practice | Menu) {
            title::scroll_backdrop(self);
        }
        briefing::preview_overlay(self);
        fade::fade(self);
        music::toggle_mute(self);
        options::apply_window(self);
        pixels::update(self);
        pointer::update(self);
        key_buttons::hover(self);
        key_buttons::level_button(self);
        hud::place_turner_marker(self);
        hud::clear_highlight(self);
        panel::show_in_play(self);
        panel::show_minimap(self);
    }
}

impl l3d_engine::Game for World {
    fn engine(&mut self) -> &mut Engine {
        &mut self.e
    }

    fn frame(&mut self) {
        self.profile.begin_frame();
        if !self.started {
            self.started = true;
            self.enter(self.state);
            self.startup();
        }
        // The screen's measurements, then the input.
        screen::update(self);
        screen::full_screen(self);
        touch::touch_over_mouse(self);
        scripted_presses(self);
        self.ui.focus(&self.e.input, &self.e.window, &self.screen);
        touch::record_touch_starts(self);
        key_buttons::press(self);
        self.change_state();
        // A step is one simulation tick, so fast-forward steps more often
        // and every sprite frame is still drawn.
        let pace = self.game.pace(self.backwards());
        self.e
            .time
            .fixed
            .set_hz(l3d_sim::TICKS_PER_SECOND as f64 * pace as f64);
        while self.e.time.fixed_step() {
            if self.state == menu::AppState::Playing {
                step_simulation(self);
            }
        }
        self.update();
        update_lemming_sprites(self);
        hdr_blink::update(self);
        sfx::play(self);
        let scale = self.e.window.scale_factor();
        self.ui.layout(&self.screen, scale);
        self.profile.cpu(profile::Cpu::Game);
        gpu::render(self);
    }
}

pub struct Data(pub GameData);

/// The level being shown; `loaded` lags `number` until the level is built.
pub struct CurrentLevel {
    number: u32,
    loaded: Option<u32>,
}

/// The preset camera last chosen (0–3), and how many times it has been
/// chosen (each time says its number).
#[derive(Default)]
pub struct PresetIndex(pub usize, pub u64);

impl PresetIndex {
    pub fn set(&mut self, i: usize) {
        self.0 = i;
        self.1 += 1;
    }
}

/// Where each of the current level's four cameras is now, starting at its
/// preset: a camera keeps its own position and facing when the player
/// switches away, and comes back to them (verified).
struct LevelInfo {
    slots: [(Vec3, f32); 4],
}

/// The player camera: free horizontal position and yaw; never pitches.
struct ViewCamera {
    pos: Vec3,
    /// Facing in radians; 0 faces −X, π/2 faces −Z (the original's rotation
    /// steps, see `docs/spec/level.md`).
    yaw: f32,
    /// Roll in radians about the view direction, turning the picture
    /// anticlockwise; 0 except in the lemming view.
    roll: f32,
    /// The lemming view's sideways look in radians from the lemming's
    /// heading, at most a quarter turn either way; positive looks right.
    head: f32,
    /// The lemming view's nod: how far the horizon is moved down the
    /// screen, as a fraction of its height (negative: up); 0 otherwise.
    nod: f32,
    /// A glide under way to another view.
    glide: Option<Glide>,
    /// Whether moves to another view glide (enhanced) or cut at once, as
    /// in the original.
    smooth: bool,
}

/// A smooth move from one view (position and yaw) to another, `t` seconds
/// in; it turns the short way round.
#[derive(Clone, Copy)]
struct Glide {
    from: (Vec3, f32),
    to: (Vec3, f32),
    t: f32,
}

/// Another camera nearer the view than this (units) is not drawn.
const CAMERA_MARKER_CLEARANCE: f32 = 0.3;

/// How long a glide to a preset view takes, in seconds (ours).
const GLIDE_SECONDS: f32 = 0.8;
/// How fast Q/E and the panel's turn arrows turn the world about the
/// level's centre, in radians per second (ours).
pub(crate) const ORBIT_SPEED: f32 = 1.2;

/// A preset camera's position and yaw.
fn preset_view(p: &CameraPreset) -> (Vec3, f32) {
    (
        Vec3::new(
            16.0 + p.x as f32 / 256.0,
            8.0 + p.y as f32 / 256.0,
            16.0 + p.z as f32 / 256.0,
        ),
        p.rotation as f32 * std::f32::consts::FRAC_PI_2,
    )
}

impl ViewCamera {
    fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Turns the world `angle` radians about the vertical through `pivot`
    /// (`[x, z]`), clockwise seen from above for positive angles: the
    /// camera circles the other way, keeping its view of the pivot.
    fn orbit(&mut self, pivot: [f32; 2], angle: f32) {
        // Rotating (x, z) by `a` turns x towards z, and a heading by the
        // same `a` (see `forward`).
        let a = -angle;
        let (s, c) = a.sin_cos();
        let (dx, dz) = (self.pos.x - pivot[0], self.pos.z - pivot[1]);
        self.pos.x = pivot[0] + dx * c - dz * s;
        self.pos.z = pivot[1] + dx * s + dz * c;
        self.yaw += a;
    }

    /// Turns the lemming view's head `by` radians to the right, at most a
    /// quarter turn either way.
    fn look(&mut self, by: f32) {
        self.head =
            (self.head + by).clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    }

    /// Jumps to a preset view.
    fn set_preset(&mut self, p: &CameraPreset) {
        (self.pos, self.yaw) = preset_view(p);
        self.glide = None;
    }

    /// Starts gliding to a preset view.
    fn glide_to_preset(&mut self, p: &CameraPreset) {
        let (pos, yaw) = preset_view(p);
        self.glide_to(pos, yaw);
    }

    /// Starts gliding to a position and yaw, or jumps there when not smooth.
    fn glide_to(&mut self, pos: Vec3, yaw: f32) {
        if !self.smooth {
            (self.pos, self.yaw) = (pos, yaw);
            self.glide = None;
            return;
        }
        self.glide = Some(Glide {
            from: (self.pos, self.yaw),
            to: (pos, yaw),
            t: 0.0,
        });
    }

    /// Moves a glide on by `dt` seconds, ending it on arrival.
    fn advance_glide(&mut self, dt: f32) {
        let Some(g) = self.glide.as_mut() else { return };
        g.t += dt;
        let s = (g.t / GLIDE_SECONDS).min(1.0);
        let s = s * s * (3.0 - 2.0 * s);
        let turn = (g.to.1 - g.from.1 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.pos = g.from.0.lerp(g.to.0, s);
        self.yaw = g.from.1 + turn * s;
        if g.t >= GLIDE_SECONDS {
            self.glide = None;
        }
    }

    /// Ends a glide at once, at its destination.
    fn finish_glide(&mut self) {
        if let Some(g) = self.glide.take() {
            (self.pos, self.yaw) = g.to;
        }
    }
}

/// Whether the camera at `p` would be inside a block that stops it: any
/// non-empty cell within [`CAMERA_RADIUS`] whose present segments reach the
/// camera's height, unless the block is flagged passable for the camera.
fn camera_blocked(level: &Level, blocks: &BlockSet, p: Vec3) -> bool {
    let r = CAMERA_RADIUS;
    for dx in [-r, r] {
        for dy in [-r, r] {
            for dz in [-r, r] {
                if let Some(b) = solid_cell(level, p + Vec3::new(dx, dy, dz))
                    && !blocks
                        .defs
                        .get(b.id as usize)
                        .is_some_and(|d| d.flags & l3d_formats::blk::flags::NON_SOLID_CAMERA != 0)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// The block at `p`, if it is there: a non-empty cell whose segment at
/// that height is present.
fn solid_cell(level: &Level, p: Vec3) -> Option<l3d_formats::level::BlockCell> {
    use l3d_formats::level::{SIZE_X, SIZE_Y, SIZE_Z};
    let c = p.floor();
    if c.min_element() < 0.0 || c.x >= SIZE_X as f32 || c.y >= SIZE_Y as f32 || c.z >= SIZE_Z as f32
    {
        return None;
    }
    let b = level.block(c.x as usize, c.y as usize, c.z as usize);
    let seg = ((p.y - c.y) * 4.0) as u8;
    (!b.is_empty() && b.segments & (1 << seg.min(3)) != 0).then_some(b)
}

/// The direction from the camera through the pointer (as last drawn), if
/// it is over the window.
pub(crate) fn pointer_ray(
    window: &Window,
    screen: &screen::Screen,
    camera: &SceneCamera,
) -> Option<Vec3> {
    screen_ray(screen, camera, screen.cursor(window)?)
}

/// The direction from the camera (as last drawn) through `p` on the screen.
pub(crate) fn screen_ray(screen: &screen::Screen, camera: &SceneCamera, p: Vec2) -> Option<Vec3> {
    let ndc = Vec2::new(
        p.x / screen.size.x * 2.0 - 1.0,
        1.0 - p.y / screen.size.y * 2.0,
    );
    let inverse = camera.view_proj.inverse();
    // Reversed depth: 1 is the near plane.
    let near = inverse.project_point3(ndc.extend(1.0));
    let far = inverse.project_point3(ndc.extend(0.1));
    (far - near).try_normalize()
}

/// The spot (`[x, z]`) the world turns about for a drag or twist along `dir`
/// from the camera at `from`: the block there, or the ground, or (with no
/// way given or nothing in it) the level's centre.
pub(crate) fn spot_under(game: &Game, from: Vec3, dir: Option<Vec3>) -> Option<[f32; 2]> {
    let hit = dir.and_then(|dir| {
        let (level, ..) = game.terrain.as_ref()?;
        first_solid(level, from, dir).or_else(|| {
            let t = (scene_build::GROUND_Y - from.y) / dir.y;
            (t > 0.0).then(|| from + dir * t)
        })
    });
    hit.map(|p| [p.x, p.z])
        .or_else(|| game.sim.as_ref().map(|sim| sim.centre()))
}

/// The first point along `dir` from `from` inside a block, if one is within
/// [`PICK_RANGE`].
fn first_solid(level: &Level, from: Vec3, dir: Vec3) -> Option<Vec3> {
    const STEP: f32 = 0.05;
    (1..=(PICK_RANGE / STEP) as usize)
        .map(|i| from + dir * i as f32 * STEP)
        .find(|&p| solid_cell(level, p).is_some())
}

/// How far the block under the pointer is looked for (grid units).
const PICK_RANGE: f32 = 128.0;

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
    // Reversed depth: the near plane at 1, the far one at 0, which keeps
    // depth precise far off, so surfaces a thousandth of a unit apart (the
    // stacked land polygons) stay apart across the level.
    let mut p = Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, 8000.0, 0.05);
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
    let centre = CENTRE_AT_POS_Z
        + (yaw - 3.0 * std::f32::consts::FRAC_PI_2) * 1024.0 / std::f32::consts::TAU;
    (centre - 160.0).rem_euclid(1024.0)
}

impl Default for ViewCamera {
    fn default() -> Self {
        ViewCamera {
            pos: Vec3::new(16.0, 8.0, 16.0),
            yaw: 0.0,
            roll: 0.0,
            head: 0.0,
            nod: 0.0,
            glide: None,
            smooth: true,
        }
    }
}

/// Whether the level about to be played is starting afresh, rather than
/// resuming after the options screen.
pub fn fresh_level(w: &World) -> bool {
    w.current.loaded.is_none()
}

/// The level being played.
#[derive(Default)]
pub struct Game {
    pub sim: Option<l3d_sim::Simulation>,
    pub paused: bool,
    /// Fast-forward: the level runs [`FAST_FORWARD_TICKS`] times as fast.
    pub fast_forward: bool,
    /// The level's pace now, in ticks per normal tick: 0 paused, 1 normal, up to
    /// [`FAST_FORWARD_TICKS`]. It rises gradually after a pause and into
    /// fast-forward, and drops at once (ours).
    speed: f32,
    /// Each lemming's place before the last tick, for drawing it
    /// moving smoothly over the step ([`Game::drawn_pos`]).
    prev_pos: Vec<[i32; 3]>,
    /// The way each lemming is drawn facing, turning towards the way it
    /// faces ([`Game::turn_yaws`]).
    yaws: Vec<f32>,
    pub save_requirement: u32,
    /// The level's block grid as currently shaped (destructible skills change
    /// it), its block dictionary, and the scene layer showing it.
    terrain: Option<(Level, BlockSet, usize)>,
    /// The level restarted as a replay of an earlier attempt, until the
    /// player takes over.
    pub replay: Option<Replay>,
    /// The exits' doors as drawn.
    doors: Vec<lemming_render::Door>,
    /// With Rewind on, the level as it was every [`SNAPSHOT_TICKS`] ticks,
    /// oldest first, for running it back.
    snapshots: Vec<l3d_sim::Simulation>,
}

/// Ticks between rewind snapshots: two seconds, about 140 KB each.
const SNAPSHOT_TICKS: u64 = 28;

/// The rating picked on the title screen is kept for the next start.
fn keep_rating(w: &mut World) {
    if w.settings.rating != w.rating.0 {
        w.settings.rating = w.rating.0;
        w.settings.save();
    }
}

/// Runs `sim` back `ticks` ticks (no further than the oldest snapshot):
/// from the latest snapshot at or before then, the player's commands are
/// applied again up to that tick, and anything later is forgotten.
fn rewind(sim: &mut l3d_sim::Simulation, snapshots: &mut Vec<l3d_sim::Simulation>, ticks: u64) {
    let target = sim.tick.saturating_sub(ticks);
    while snapshots.len() > 1 && snapshots.last().is_some_and(|s| s.tick > target) {
        snapshots.pop();
    }
    let Some(snap) = snapshots.last() else {
        return;
    };
    let target = target.max(snap.tick);
    let mut back = snap.clone();
    while back.tick < target {
        back.step();
        let now = back.tick;
        for &(_, c) in sim.log.iter().filter(|(t, _)| *t == now) {
            back.apply(c);
        }
    }
    // Their sounds were heard the first time.
    back.take_events();
    *sim = back;
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
        Replay {
            log,
            next: 0,
            logged: 0,
            demo: None,
        }
    }

    pub fn demo(log: Vec<(u64, l3d_sim::Command)>, back_to: menu::AppState) -> Self {
        Replay {
            demo: Some(back_to),
            ..Replay::new(log)
        }
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

/// How many times as fast the level runs while fast-forwarding (the original's speed-up
/// is unmeasured).
const FAST_FORWARD_TICKS: u32 = 3;
/// Seconds for the game to reach normal speed after a pause, and again for
/// fast-forward to reach its speed.
const SPEED_RAMP_SECONDS: f32 = 0.5;

/// P or Space pauses and resumes the level, and Enter turns fast-forward on
/// and off (not during the preview, which keeps it waiting itself, nor while
/// the restart question is open). Coming back from the background pauses it.
fn pause_keys(w: &mut World) {
    let (keys, game) = (&w.e.input.keys, &mut w.game);
    if w.preview.active || w.confirm.question.is_some() {
        return;
    }
    if keys.any_just_pressed([KeyCode::KeyP, KeyCode::Space]) {
        game.paused = !game.paused;
    }
    // Back from the background (Android), the level waits to be resumed.
    if w.e.input.resumed {
        game.paused = true;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        game.fast_forward = !game.fast_forward;
    }
}

fn step_simulation(w: &mut World) {
    let backwards = w.backwards();
    let (options, scene) = (&w.opts, &mut w.scene);
    let keep_snapshots = w.settings.rewind;
    // A turner half given (one click) may be to a lemming not yet out then.
    if backwards {
        w.pending.0 = None;
    }
    // The level waits while the camera glides to a preset view, or the
    // picture changes resolution, so neither costs the player time.
    if w.pixelation.changing() || w.view.glide.is_some() {
        // Waiting, lemmings are drawn where they stand.
        if let Some(sim) = &w.game.sim {
            w.game.prev_pos.clear();
            w.game.prev_pos.extend(sim.lemmings.iter().map(|l| l.pos));
        }
        return;
    }
    let Game {
        sim: Some(sim),
        paused,
        terrain,
        fast_forward,
        speed,
        prev_pos,
        replay,
        doors,
        snapshots,
        ..
    } = &mut w.game
    else {
        return;
    };
    // Speeding up gradually, after a pause and into fast-forward, and
    // stopping or slowing at once, so the player keeps control; the ticks
    // are the same ones, so only their pace changes.
    let target = match (*paused, *fast_forward) {
        (true, _) => 0.0,
        (false, false) => 1.0,
        (false, true) => FAST_FORWARD_TICKS as f32,
    };
    // A step lasts a tick at the current pace.
    let rise = (FAST_FORWARD_TICKS - 1) as f32
        / (SPEED_RAMP_SECONDS * l3d_sim::TICKS_PER_SECOND as f32 * speed.max(1.0));
    // Out of a pause straight to normal speed (a gradual start showed
    // lemmings gliding while their poses stood still, owner).
    *speed = if target < *speed || *speed < 1.0 {
        target.min(1.0)
    } else {
        (*speed + rise).min(target)
    };
    // Where the lemmings stand before this step's tick; paused, where they
    // are, so that nothing drifts.
    prev_pos.clear();
    prev_pos.extend(sim.lemmings.iter().map(|l| l.pos));
    if !keep_snapshots {
        snapshots.clear();
    }
    // Backspace runs the level back at the pace it would run forward
    // (ours), paused or not; a replay stops there and the player takes over.
    if backwards && sim.tick > snapshots.first().map_or(u64::MAX, |s| s.tick) {
        *replay = None;
        let before = sim.world.clone();
        rewind(sim, snapshots, 1);
        sim.world.take_changes();
        sim.world.take_bricks_changed();
        if before.same_terrain(&sim.world) && sim.tick > l3d_sim::HATCH_OPEN_TICKS as u64 {
            return;
        }
        let (Some((level, blocks, layer)), Some(current)) = (terrain, &scene.data) else {
            return;
        };
        // Only the cells the rewind changed: blocks lemmings pass through
        // are not in the simulation's world, and stay as they are.
        for x in 0..l3d_formats::level::SIZE_X {
            for y in 0..l3d_formats::level::SIZE_Y {
                for z in 0..l3d_formats::level::SIZE_Z {
                    let c = [x as i32, y as i32, z as i32];
                    let cell = sim.world.block(c);
                    if cell != before.block(c) {
                        level.set_block(x, y, z, cell.map(|(b, _)| b).unwrap_or_default());
                    }
                }
            }
        }
        let rebuilt = scene_build::rebuild_blocks(current, *layer, level, blocks, sim);
        scene.version += 1;
        scene.data = Some(Arc::new(rebuilt));
        return;
    }
    if *paused {
        return;
    }
    if keep_snapshots
        && sim.tick.is_multiple_of(SNAPSHOT_TICKS)
        && snapshots.last().is_none_or(|s| s.tick < sim.tick)
    {
        snapshots.push(sim.clone());
    }
    if replay.as_mut().is_some_and(|r| !r.feed(sim)) {
        *replay = None;
    }
    sim.step();
    lemming_render::step_doors(doors, sim);
    for &step in &options.assign {
        if step.0 == sim.tick && replay.is_none() {
            l3d_sim::demos::apply_step(sim, step);
        }
    }
    let changes = sim.world.take_changes();
    let bricks_changed = sim.world.take_bricks_changed();
    // The hatches swing open over the first ticks.
    let opening = sim.tick <= l3d_sim::HATCH_OPEN_TICKS as u64;
    if changes.is_empty() && !bricks_changed && !opening {
        return;
    }
    let (Some((level, blocks, layer)), Some(current)) = (terrain, &scene.data) else {
        return;
    };
    for ([x, y, z], cell) in changes {
        level.set_block(x, y, z, cell);
    }
    let rebuilt = scene_build::rebuild_blocks(current, *layer, level, blocks, sim);
    scene.version += 1;
    scene.data = Some(Arc::new(rebuilt));
}

impl Game {
    /// Ticks per normal tick at which the level runs, forwards or back.
    fn pace(&self, backwards: bool) -> f32 {
        match (backwards, self.fast_forward) {
            (true, true) => FAST_FORWARD_TICKS as f32,
            (true, false) => 1.0,
            (false, _) => self.speed.max(1.0),
        }
    }

    /// How far the game is through its current step towards the next, 0
    /// to 1, for drawing lemmings between their places; 1 (where they are)
    /// in Original mode, which moves them in steps as the original did.
    fn tick_progress(&self, enhanced: bool, overstep: f32) -> f32 {
        if enhanced {
            overstep.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Where lemming `i` is drawn, in units: between its places before and
    /// after the last step's ticks, `f` of the way, so that it moves
    /// smoothly at the screen's frame rate rather than in the simulation's
    /// steps (ours). Only a lemming that really goes from one place to
    /// another, out of a hatch or through a teleporter, is drawn where it is.
    fn drawn_pos(&self, sim: &l3d_sim::Simulation, i: usize, f: f32) -> [f32; 3] {
        let l = &sim.lemmings[i];
        match self.prev_pos.get(i) {
            Some(before) if !matches!(l.state, l3d_sim::State::Teleporting { .. }) => {
                std::array::from_fn(|k| {
                    crate::units(before[k]) + (crate::units(l.pos[k]) - crate::units(before[k])) * f
                })
            }
            _ => l.pos.map(crate::units),
        }
    }
}

/// How fast a lemming is drawn turning, radians per second of game time: a
/// quarter turn in a quarter of a second, through the diagonal views.
const TURN_RATE: f32 = std::f32::consts::FRAC_PI_2 / 0.25;

impl Game {
    /// Turns each lemming's drawn facing towards the way it faces, by `dt`
    /// seconds of game time at the game's speed (none while paused); at once
    /// in Original mode, and for lemmings just released.
    fn turn_yaws(&mut self, enhanced: bool, dt: f32) {
        use std::f32::consts::{PI, TAU};
        let Some(sim) = &self.sim else { return };
        let step = TURN_RATE * dt * self.speed;
        for (i, l) in sim.lemmings.iter().enumerate() {
            let target = lemming_render::facing(l).yaw();
            let Some(yaw) = self.yaws.get_mut(i) else {
                self.yaws.push(target);
                continue;
            };
            let diff = (target - *yaw + PI).rem_euclid(TAU) - PI;
            *yaw = if enhanced && diff.abs() > step {
                (*yaw + step * diff.signum()).rem_euclid(TAU)
            } else {
                target
            };
        }
    }
}

fn update_lemming_sprites(w: &mut World) {
    let dt = w.e.time.real.delta_secs();
    w.game.turn_yaws(w.settings.enhanced, dt);
    let f = w
        .game
        .tick_progress(w.settings.enhanced, w.e.time.fixed.overstep_fraction());
    let (game, lemming_cam, highlight) = (&w.game, &w.lemming_cam, &w.highlight);
    let (info, preset, sprites) = (&w.info, &w.preset, &mut w.sprites);
    let Some(sim) = &game.sim else { return };
    let cams = std::iter::once(&w.view);
    let yaw = w.view.yaw;
    // The other three cameras where they are now (verified); the current
    // one is the view. One right at the view would fill it, so is left out
    // (ours).
    let eye = Some(w.view.pos);
    let cameras: Vec<_> = info
        .iter()
        .flat_map(|info| info.slots.iter().enumerate())
        .filter(|(i, (pos, _))| {
            *i != preset.0 && eye.is_none_or(|e| e.distance(*pos) > CAMERA_MARKER_CLEARANCE)
        })
        .map(|(i, &(pos, yaw))| lemming_render::CameraMarker {
            number: i as u32 + 1,
            pos,
            yaw,
        })
        .collect();
    // The lemming ridden with is the view's eyes, drawn until the camera
    // has glided into them.
    let eyes = lemming_cam
        .following()
        .filter(|_| cams.clone().all(|c| c.glide.is_none()));
    let blocks = game.terrain.as_ref().map(|(_, blocks, _)| blocks);
    let drawn: Vec<([f32; 3], f32)> = (0..sim.lemmings.len())
        .map(|i| {
            (
                game.drawn_pos(sim, i, f),
                game.yaws.get(i).copied().unwrap_or(0.0),
            )
        })
        .collect();
    *sprites = lemming_render::build(
        sim,
        &drawn,
        blocks,
        scene_build::ATLAS_ROWS,
        yaw,
        w.view.pos,
        &game.doors,
        eyes,
        highlight.lemming,
        &cameras,
    );
}

pub(crate) fn load_level(w: &mut World) {
    let (current, data, opts, scene, game) = (
        &mut w.current,
        &mut w.data,
        &w.opts,
        &mut w.scene,
        &mut w.game,
    );
    let (audio, music, muted, settings) = (&w.e.audio, &mut w.music, &w.muted, &w.settings);
    if current.loaded == Some(current.number) {
        return;
    }
    let n = current.number;
    current.loaded = Some(n);
    let scene_build::BuiltLevel {
        level,
        blocks,
        scene: content,
        mesh,
        block_layer,
    } = match scene_build::build(&mut data.0, n, settings.show()) {
        Ok(v) => v,
        Err(e) => {
            error!("level {n}: {e}");
            return;
        }
    };
    if !mesh.unsupported_shapes.is_empty() {
        warn!(
            "level {n}: unsupported shapes (shape, cells): {:?}",
            mesh.unsupported_shapes
        );
    }
    info!(
        "level {n:03} {:?}: texture set {}, {} opaque + {} cutout triangles",
        level.title,
        level.texture_set,
        mesh.opaque.indices.len() / 3,
        mesh.cutout.indices.len() / 3
    );
    w.e.window.title = format!("openlem3d - LEVEL.{n:03} {}", level.title);
    scene.version += 1;
    scene.data = Some(Arc::new(content));
    let preset = opts.camera.filter(|c| (1..=4).contains(c)).unwrap_or(1) - 1;
    w.preset.set(preset);
    game.fast_forward = false;
    game.speed = 1.0;
    game.prev_pos.clear();
    game.yaws.clear();
    game.paused = false;
    // A restart starts afresh: not riding along, nothing highlighted.
    w.lemming_cam = lemming_cam::LemmingCam::Off;
    w.pending = hud::PendingTurner(None);
    w.highlight = hud::Highlight::default();
    let cam = &mut w.view;
    cam.set_preset(&level.cameras[preset]);
    if let Some((pos, yaw)) = opts.look {
        (cam.pos, cam.yaw) = (pos, yaw);
    }
    w.info = Some(LevelInfo {
        slots: level.cameras.each_ref().map(preset_view),
    });
    game.sim = Some(l3d_sim::Simulation::new(&level, &blocks));
    game.snapshots.clear();
    if opts.solve
        && let Some(log) = l3d_sim::demos::demo(n, &level, &blocks)
    {
        game.replay = Some(Replay::new(log));
    }
    game.doors = lemming_render::doors(&level, &blocks);
    game.save_requirement = level.save_requirement as u32;
    let track = music::track_for(level.theme, level.music);
    music::play_track(
        music,
        audio,
        &data.0.disc,
        track,
        music::volume(muted, settings),
    );
    game.terrain = Some((level, blocks, block_layer));
    panel::select_first_skill(w);
}

/// What the camera controls keep between frames: where the camera last
/// stood clear of blocks, the pointer travel of the two buttons held, the
/// keys' eased moves and the spot a middle-button drag turns about.
#[derive(Default)]
struct CameraControls {
    last_free: Option<Vec3>,
    held: [Option<f32>; 2],
    eased: Vec4,
    pivot: Option<[f32; 2]>,
}

pub(crate) fn camera_controls(w: &mut World) {
    let opts = &w.opts;
    let input = &w.e.input;
    let (keys, mouse, motion, scroll) = (&input.keys, &input.mouse, &input.motion, &input.scroll);
    let time = w.e.time.game;
    let info = &mut w.info;
    let (window, screen) = (&w.e.window, &w.screen);
    let scene_cam = &mut w.camera;
    let (lemming_cam, preview) = (&w.lemming_cam, &w.preview);
    let (preset, settings, game) = (&mut w.preset, &w.settings, &w.game);
    let cc = &mut w.camera_controls;
    let (last_free, held, eased, pivot) = (
        &mut cc.last_free,
        &mut cc.held,
        &mut cc.eased,
        &mut cc.pivot,
    );
    let trackpad = &w.trackpad;
    let ui_active = w.ui.any_active();
    // Pointer travel since a button went down over the view (not the
    // panel), while it is held.
    let track = |held: &mut Option<f32>, button: MouseButton| {
        if mouse.just_pressed(button) {
            *held = (!ui_active).then_some(0.0);
        }
        if !mouse.pressed(button) {
            *held = None;
        }
        if let Some(d) = held.as_mut() {
            *d += motion.delta.length();
        }
        held.is_some_and(|d| d > DRAG_SLOP)
    };
    let [held_drag, action_drag] = held;
    let enhanced = settings.enhanced;
    let dragging = track(held_drag, settings.turn_button()) && enhanced;
    let action_dragging = track(action_drag, settings.action_button()) && enhanced;
    // Enhanced (ours, as touch): an action-button drag turns the view and
    // moves it forward and back, a camera-button drag moves it sideways and
    // up and down, and the wheel moves it towards the pointer and back.
    // Original: holding the camera button moves the camera as the arrow
    // under the pointer shows (see `pointer`; verified: a drag does not turn
    // the view, the arrow just follows the pointer), and the wheel raises and
    // lowers it.
    let region = held_drag
        .as_ref()
        .filter(|_| !enhanced)
        .and(screen.cursor(window).map(|p| (p, screen.size)))
        .map(|(p, size)| pointer_region(p, size));
    // The way through the pointer, for the wheel (Enhanced).
    let toward = pointer_ray(window, screen, scene_cam);
    // Riding along with a lemming: no manual movement.
    let dt = if lemming_cam.following().is_some() {
        0.0
    } else {
        time.delta_secs()
    };
    for cam in std::iter::once(&mut w.view) {
        cam.smooth = settings.enhanced;
        if !cam.smooth {
            cam.finish_glide();
        }
        let presets = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
        ];
        if let Some(info) = info.as_ref() {
            for (i, k) in presets.iter().enumerate() {
                if keys.just_pressed(*k) {
                    let (pos, yaw) = info.slots[i];
                    cam.glide_to(pos, yaw);
                    preset.set(i);
                }
            }
        }
        let gliding = cam.glide.is_some();
        let key = |k: KeyCode| keys.pressed(k) as i32 as f32;
        let held = |a: KeyCode, b: KeyCode| (keys.pressed(a) || keys.pressed(b)) as i32 as f32;
        // ← → turn the camera; Q and E turn the world about the level's
        // centre, like the panel's turn arrows (owner: the arrows turn the
        // whole level, not the camera).
        let arrows = key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft);
        let world_turn = key(KeyCode::KeyE) - key(KeyCode::KeyQ);
        let turn = arrows + world_turn;
        // The keys held: forward, sideways, up, turning. In Enhanced mode
        // (ours) the camera eases into and out of their moves; the
        // original's moves start and stop at once.
        let pressed = Vec4::new(
            held(KeyCode::KeyW, KeyCode::ArrowUp) - held(KeyCode::KeyS, KeyCode::ArrowDown),
            key(KeyCode::KeyD) - key(KeyCode::KeyA),
            held(KeyCode::KeyR, KeyCode::PageUp) - held(KeyCode::KeyF, KeyCode::PageDown),
            arrows,
        );
        if !enhanced || lemming_cam.following().is_some() || gliding {
            *eased = pressed;
        } else {
            *eased = eased.lerp(pressed, 1.0 - (-dt / KEY_EASE_SECONDS).exp());
            if eased.abs().max_element() < 1e-3 {
                *eased = pressed;
            }
        }
        cam.yaw += eased.w * 1.8 * dt;
        if world_turn != 0.0
            && lemming_cam.following().is_none()
            && let Some(sim) = game.sim.as_ref()
        {
            cam.orbit(sim.centre(), world_turn * ORBIT_SPEED * dt);
        }
        // Drags, as for touch (see `touch`): the world follows the pointer.
        let mut drag = Vec3::ZERO;
        if dt > 0.0 && action_dragging {
            cam.yaw += motion.delta.x * touch::TURN_PER_PX;
            drag += cam.forward() * motion.delta.y * touch::MOVE_PER_PX;
        }
        if dt > 0.0 && dragging {
            let fwd = cam.forward();
            let right = Vec3::new(-fwd.z, 0.0, fwd.x);
            drag += (-right * motion.delta.x + Vec3::Y * motion.delta.y) * touch::MOVE_PER_PX;
        }
        // A middle-button drag (Enhanced, ours) turns the world about the
        // block under the pointer where it began (or the ground there, or
        // the level's centre), as if dragging it round, and moves the camera
        // up and down as a camera-button drag does.
        if !mouse.pressed(MouseButton::Middle) || !enhanced || dt == 0.0 {
            *pivot = None;
        } else if mouse.just_pressed(MouseButton::Middle) && !ui_active {
            *pivot = spot_under(game, cam.pos, toward);
        }
        let orbiting = pivot.is_some() && motion.delta != Vec2::ZERO;
        if let Some(p) = *pivot {
            cam.orbit(p, -motion.delta.x * touch::TURN_PER_PX);
            drag += Vec3::Y * motion.delta.y * touch::MOVE_PER_PX;
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
        // Riding along, the same keys and regions turn the head instead, as
        // do A and D and a sideways drag with any button (enhanced).
        if lemming_cam.following().is_some() {
            let sideways = key(KeyCode::KeyD) - key(KeyCode::KeyA);
            let mut look = (turn + spin + sideways).clamp(-1.0, 1.0)
                * lemming_cam::HEAD_SPEED
                * time.delta_secs();
            let middle_dragging = enhanced && mouse.pressed(MouseButton::Middle) && !ui_active;
            if action_dragging || dragging || middle_dragging {
                look += motion.delta.x * touch::TURN_PER_PX;
            }
            cam.look(look);
        } else {
            cam.head = 0.0;
        }
        let fwd = cam.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            16.0
        } else {
            CAMERA_SPEED
        } * settings.camera_factor();
        let mv = fwd * eased.x + right * eased.y + Vec3::Y * eased.z + fwd * ahead + right * side;
        cam.pos += mv * speed * dt + drag;
        // Scrolling that a trackpad gesture makes moves the camera through
        // its fingers instead (`trackpad`).
        let notches = match scroll.unit {
            _ if trackpad.owns_wheel() => 0.0,
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 50.0,
        };
        if dt > 0.0 {
            cam.pos += notches
                * if enhanced {
                    toward.unwrap_or(fwd) * WHEEL_STEP_AHEAD
                } else {
                    Vec3::Y * WHEEL_STEP
                };
        }
        // Moving the camera by hand stops a glide where it is.
        let manual = dt > 0.0
            && (turn != 0.0
                || spin != 0.0
                || dragging
                || action_dragging
                || pressed.truncate() != Vec3::ZERO
                || orbiting
                || ahead != 0.0
                || side != 0.0
                || notches != 0.0);
        if manual {
            cam.glide = None;
        } else {
            cam.advance_glide(time.delta_secs());
        }
        // Never below the ground plane (the sea and land).
        cam.pos.y = cam.pos.y.max(scene_build::GROUND_Y + CAMERA_RADIUS);
        // The camera can't enter blocks (unless they are marked passable for
        // it); it slides along them. Preset jumps and glides, the lemming
        // view and the preview are exempt; after a glide the camera moves
        // freely until it is clear of blocks.
        if gliding {
            *last_free = None;
        }
        if let Some((level, blocks, _)) = &game.terrain {
            let jumped = keys.any_just_pressed(presets)
                || lemming_cam.following().is_some()
                || preview.active
                || gliding;
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
        // The current camera keeps where the player leaves it (not the
        // lemming view, a glide or the preview).
        if let Some(info) = info.as_mut()
            && lemming_cam.following().is_none()
            && cam.glide.is_none()
            && !preview.active
        {
            info.slots[preset.0] = (cam.pos, cam.yaw);
        }
        let aspect = screen.size.x / screen.size.y.max(1.0);
        let view = Mat4::look_to_rh(cam.pos, cam.forward(), Vec3::Y);
        let horizon = opts.horizon + cam.nod;
        scene_cam.view_proj = roll_about_horizon(cam.roll, aspect, horizon)
            * projection(opts.fov_y, aspect, horizon)
            * view;
        scene_cam.roll = cam.roll;
        scene_cam.horizon = horizon;
        scene_cam.time = time.elapsed_secs();
        scene_cam.sky_column = sky_left_column(cam.yaw);
        scene_cam.right = right;
    }
}

/// With `--screenshot`, saves one frame `--wait` seconds after the level has
/// loaded, then exits.
/// What `--screenshot` keeps between frames: the game time counted, the
/// frames since the shot, and the tick last recorded.
#[derive(Default)]
struct ShotTimer {
    elapsed: Option<f32>,
    frames_since_shot: Option<u32>,
    last_recorded: Option<u64>,
}

fn screenshot_when_ready(w: &mut World) {
    let (screenshots, opts, current) = (&mut w.screenshots, &w.opts, &w.current);
    let (time, game) = (w.e.time.game, &w.game);
    let ShotTimer {
        elapsed,
        frames_since_shot,
        last_recorded,
    } = &mut w.shot;
    let Some(path) = &opts.screenshot else { return };
    if current.loaded.is_none()
        && !matches!(
            w.state,
            menu::AppState::Menu
                | menu::AppState::Title
                | menu::AppState::Code
                | menu::AppState::Briefing
                | menu::AppState::Options
                | menu::AppState::Practice
        )
    {
        return;
    }
    let before = elapsed.unwrap_or(-time.delta_secs());
    let now = before + time.delta_secs();
    *elapsed = Some(now);
    let shot_at = opts.wait.max(0.5);
    if let (Some((dir, from)), Some(sim)) = (&opts.record, &game.sim)
        && now >= *from
        && now < shot_at
        && *last_recorded != Some(sim.tick)
    {
        *last_recorded = Some(sim.tick);
        let _ = std::fs::create_dir_all(dir);
        screenshots.0.push(dir.join(format!("{:05}.png", sim.tick)));
    }
    if before < shot_at && now >= shot_at {
        screenshots.0.push(path.clone());
        *frames_since_shot = Some(0);
    }
    // Quit a few frames after the shot, once it is saved (counted in frames,
    // not game time, which `--turbo` races through).
    if let Some(f) = frames_since_shot.as_mut() {
        *f += 1;
        if *f > 30 {
            w.e.exit();
        }
    }
}

/// With `--no-hud`, hides every top-level UI node.
fn hide_ui(w: &mut World) {
    if !w.opts.no_hud {
        return;
    }
    for root in w.ui.roots().collect::<Vec<_>>() {
        w.ui.set_visible(root, false);
    }
}

/// Rebuilds the scenery when the land, sea or sky setting changes during a
/// level (on the options screen), keeping the terrain as the lemmings left it.
fn refresh_scenery(w: &mut World) {
    let (settings, current, data) = (&w.settings, &w.current, &mut w.data);
    let (scene, game, shown) = (&mut w.scene, &mut w.game, &mut w.scenery_shown);
    let Some(n) = current.loaded else { return };
    let show = settings.show();
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
    let Game { sim, terrain, .. } = game;
    let content = match (terrain.as_mut(), sim) {
        (Some((level, blocks, layer)), Some(sim)) => {
            *layer = built.block_layer;
            scene_build::rebuild_blocks(&built.scene, built.block_layer, level, blocks, sim)
        }
        _ => built.scene,
    };
    scene.version += 1;
    scene.data = Some(Arc::new(content));
}

/// Presses the keys given with `--press SECONDS:KEY[:HOLD]` at their times,
/// for HOLD seconds or one frame: key names as in Bevy's `KeyCode`
/// (`Escape`, `F12`, `KeyW`, `Digit1`, …), or `Click` for the left mouse button.
/// The scripted presses made so far, and the keys they hold with until
/// when.
#[derive(Default)]
struct Scripted {
    done: usize,
    held: Vec<(String, f32)>,
}

fn scripted_presses(w: &mut World) {
    let (opts, time) = (&w.opts, w.e.time.game);
    let (keys, mouse) = (&mut w.e.input.keys, &mut w.e.input.mouse);
    let Scripted { done, held } = &mut w.scripted;
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
        "Tab" => KeyCode::Tab,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
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
        "AltLeft" => KeyCode::AltLeft,
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

/// Pointer travel (logical pixels) after which holding a mouse button is a
/// drag rather than a hold or a click.
pub(crate) const DRAG_SLOP: f32 = 6.0;
/// Camera speed with the keys and arrows (grid units per second).
pub const CAMERA_SPEED: f32 = 6.0;
/// Camera rise per wheel notch (grid units).
const WHEEL_STEP: f32 = 0.25;
/// How quickly the camera eases into and out of the keys' moves in Enhanced
/// mode: the time constant, in seconds (ours).
const KEY_EASE_SECONDS: f32 = 0.12;
/// Camera travel forward per wheel notch in Enhanced mode (grid units).
const WHEEL_STEP_AHEAD: f32 = 0.5;

/// The cell `[column, row]` of the 3×3 grid over the view that `p` is in.
pub fn pointer_region(p: Vec2, size: Vec2) -> [usize; 2] {
    [
        ((p.x / size.x * 3.0) as usize).min(2),
        ((p.y / size.y * 3.0) as usize).min(2),
    ]
}

/// With `--turbo`, game time runs up to a thousand times real time, up to
/// ten seconds of it per frame: the simulation steps as fast as the machine
/// allows (rendering only now and then). The limit is on the frame's real
/// time (10 ms, a thousand times over); `--wait` and `--press` land within
/// a frame of their time.
fn turbo(w: &mut World) {
    let time = &mut w.e.time.game;
    if w.opts.turbo {
        time.set_max_delta(std::time::Duration::from_millis(10));
        time.set_relative_speed(1000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With reversed depth, nearer is deeper, and a ray through the screen
    /// still points away from the camera.
    #[test]
    fn reversed_depth() {
        let pos = Vec3::new(3.0, 4.0, 5.0);
        let forward = Vec3::new(1.0, -0.5, -1.0).normalize();
        let camera = SceneCamera {
            view_proj: projection(60.0, 16.0 / 9.0, 0.5) * Mat4::look_to_rh(pos, forward, Vec3::Y),
            ..default()
        };
        let depth = |d: f32| camera.view_proj.project_point3(pos + forward * d).z;
        assert!(depth(1.0) > depth(10.0) && depth(10.0) > depth(1000.0));
        let screen = screen::Screen {
            size: Vec2::new(1600.0, 900.0),
            ..default()
        };
        let ray = screen_ray(&screen, &camera, Vec2::new(800.0, 450.0)).unwrap();
        assert!(ray.dot(forward) > 0.999, "{ray}");
    }
}
