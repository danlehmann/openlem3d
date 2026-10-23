//! The event loop: opens the window, takes in what happens to it, and runs
//! a frame each time round, as fast as drawing lets it. On Android the
//! window can be drawn into only between `resumed` and `suspended`; no
//! frames run outside that.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{UVec2, Vec2};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Fullscreen, WindowId};

use crate::engine::{Engine, Game};
use crate::input::{self, KeyboardInput, MouseScrollUnit, MouseWheel, Raw};
use crate::window::{CursorIcon, WindowHandle, WindowMode};

/// What the system's window was last told, to tell it only of changes.
#[derive(Default, PartialEq)]
struct Applied {
    title: String,
    mode: WindowMode,
    ime: bool,
    cursor: Option<CursorIcon>,
}

struct Runner<G: Game> {
    game: G,
    window: Option<Arc<winit::window::Window>>,
    /// Whether the window can be drawn into.
    active: bool,
    events: Vec<Raw>,
    applied: Applied,
    /// Pointer pictures made so far, by key and size.
    cursors: HashMap<(u64, u16, u16), winit::window::CustomCursor>,
}

pub(crate) fn run<G: Game>(game: G) {
    #[allow(unused_mut)]
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "android")]
    {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        builder.with_android_app(crate::android::app().clone());
    }
    let event_loop = builder.build().expect("cannot start the event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner {
        game,
        window: None,
        active: false,
        events: Vec::new(),
        applied: Applied::default(),
        cursors: HashMap::new(),
    };
    if let Err(e) = event_loop.run_app(&mut runner) {
        log::error!("event loop: {e}");
    }
}

impl<G: Game> Runner<G> {
    fn engine(&mut self) -> &mut Engine {
        self.game.engine()
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) {
        let settings = self.engine().window.clone();
        let mut attributes =
            winit::window::Window::default_attributes().with_title(&settings.title);
        attributes = match settings.resolution.requested() {
            Some(s) => attributes.with_inner_size(PhysicalSize::new(s.x, s.y)),
            None => attributes.with_inner_size(LogicalSize::new(1280.0, 720.0)),
        };
        if settings.mode != WindowMode::Windowed {
            attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = match event_loop.create_window(attributes) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("cannot open the window: {e}");
                event_loop.exit();
                return;
            }
        };
        self.applied.title = settings.title;
        self.applied.mode = settings.mode;
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let w = &mut self.engine().window;
        w.resolution
            .set_physical(UVec2::new(size.width, size.height));
        w.resolution.set_scale_factor(scale);
        self.window = Some(window);
    }

    /// Runs a frame, then tells the system's window of what it changed.
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let events = std::mem::take(&mut self.events);
        let engine = self.engine();
        input::take_in(&mut engine.input, events);
        engine.time.advance();
        engine.images.update();
        self.game.frame();
        self.apply(event_loop);
        if self.engine().should_exit() {
            event_loop.exit();
        }
    }

    fn apply(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let engine = self.game.engine();
        let w = engine.window.clone();
        let icon = Some(engine.cursor.clone());
        if w.title != self.applied.title {
            window.set_title(&w.title);
            self.applied.title = w.title.clone();
        }
        if w.mode != self.applied.mode {
            window.set_fullscreen(match w.mode {
                WindowMode::Windowed => None,
                WindowMode::BorderlessFullscreen(_) => Some(Fullscreen::Borderless(None)),
            });
            self.applied.mode = w.mode;
        }
        if w.ime_enabled != self.applied.ime {
            window.set_ime_allowed(w.ime_enabled);
            self.applied.ime = w.ime_enabled;
        }
        if icon != self.applied.cursor {
            match &icon {
                None => window.set_cursor(winit::window::CursorIcon::Default),
                Some(CursorIcon::System(i)) => window.set_cursor(*i),
                Some(CursorIcon::Custom(c)) => {
                    let key = (c.key, c.width, c.height);
                    let cursor = match self.cursors.get(&key) {
                        Some(cursor) => Some(cursor.clone()),
                        None => winit::window::CustomCursor::from_rgba(
                            c.rgba.to_vec(),
                            c.width,
                            c.height,
                            c.hotspot.0,
                            c.hotspot.1,
                        )
                        .inspect_err(|e| log::warn!("pointer picture: {e}"))
                        .ok()
                        .map(|source| {
                            let cursor = event_loop.create_custom_cursor(source);
                            self.cursors.insert(key, cursor.clone());
                            cursor
                        }),
                    };
                    if let Some(cursor) = cursor {
                        window.set_cursor(cursor);
                    }
                }
            }
            self.applied.cursor = icon;
        }
    }
}

impl<G: Game> ApplicationHandler for Runner<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.open(event_loop);
        }
        let Some(window) = self.window.clone() else {
            return;
        };
        self.active = true;
        let engine = self.engine();
        engine.handle = Some(WindowHandle(window));
        engine.handle_generation += 1;
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.active = false;
        self.engine().handle = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self
                .engine()
                .window
                .resolution
                .set_physical(UVec2::new(size.width, size.height)),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self
                .engine()
                .window
                .resolution
                .set_scale_factor(scale_factor as f32),
            WindowEvent::Focused(focused) => {
                self.engine().window.focused = focused;
                self.events.push(Raw::Focus(focused));
            }
            WindowEvent::CursorMoved { position, .. } => self
                .engine()
                .window
                .set_physical_cursor_position(Some(Vec2::new(
                    position.x as f32,
                    position.y as f32,
                ))),
            WindowEvent::CursorLeft { .. } => {
                self.engine().window.set_physical_cursor_position(None)
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.events.push(Raw::MouseButton(button, state));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (unit, x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (MouseScrollUnit::Line, x, y),
                    MouseScrollDelta::PixelDelta(p) => {
                        (MouseScrollUnit::Pixel, p.x as f32, p.y as f32)
                    }
                };
                self.events.push(Raw::Wheel(MouseWheel { unit, x, y }));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.events.push(Raw::Key(KeyboardInput {
                    key_code: match event.physical_key {
                        PhysicalKey::Code(code) => Some(code),
                        PhysicalKey::Unidentified(_) => None,
                    },
                    logical_key: event.logical_key,
                    state: event.state,
                    text: event.text,
                    repeat: event.repeat,
                }));
            }
            WindowEvent::Touch(t) => {
                let scale = self.engine().window.scale_factor();
                let p = Vec2::new(t.location.x as f32, t.location.y as f32) / scale;
                self.events.push(Raw::Touch(t.id, t.phase, p));
            }
            WindowEvent::PinchGesture { delta, .. } => self.events.push(Raw::Pinch(delta as f32)),
            WindowEvent::PanGesture { delta, .. } => {
                self.events.push(Raw::Pan(Vec2::new(delta.x, delta.y)));
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.events
                .push(Raw::MouseMotion(Vec2::new(delta.0 as f32, delta.1 as f32)));
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.active {
            self.frame(event_loop);
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        // Let go of the window's surface before the window.
        self.engine().handle = None;
    }
}
