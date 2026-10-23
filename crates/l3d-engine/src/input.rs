//! Keyboard, mouse, touch and gesture input, as resources and messages in
//! the shapes Bevy gave them. Everything that happened since the last frame
//! is taken in before it starts, so systems see it from [`First`] on.
//!
//! [`First`]: crate::app::First

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use bevy_ecs::message::Message;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;
use glam::Vec2;
pub use winit::event::{ElementState as ButtonState, MouseButton, TouchPhase};
pub use winit::keyboard::{Key, KeyCode, NamedKey, NativeKey, SmolStr};

use crate::app::App;

/// Where the frame's input is taken in: before every schedule, so ordering
/// after it changes nothing (kept for the systems that say so).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputSystems;

/// Which of some buttons are held, and which went down or up since the
/// last frame.
#[derive(Resource, Debug, Clone)]
pub struct ButtonInput<T: Copy + Eq + Hash + Send + Sync + 'static> {
    pressed: HashSet<T>,
    just_pressed: HashSet<T>,
    just_released: HashSet<T>,
}

impl<T: Copy + Eq + Hash + Send + Sync + 'static> Default for ButtonInput<T> {
    fn default() -> Self {
        ButtonInput {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
        }
    }
}

impl<T: Copy + Eq + Hash + Send + Sync + 'static> ButtonInput<T> {
    pub fn press(&mut self, input: T) {
        if self.pressed.insert(input) {
            self.just_pressed.insert(input);
        }
    }

    pub fn release(&mut self, input: T) {
        if self.pressed.remove(&input) {
            self.just_released.insert(input);
        }
    }

    pub fn release_all(&mut self) {
        self.just_released.extend(self.pressed.drain());
    }

    pub fn pressed(&self, input: T) -> bool {
        self.pressed.contains(&input)
    }

    pub fn any_pressed(&self, inputs: impl IntoIterator<Item = T>) -> bool {
        inputs.into_iter().any(|i| self.pressed(i))
    }

    pub fn just_pressed(&self, input: T) -> bool {
        self.just_pressed.contains(&input)
    }

    pub fn any_just_pressed(&self, inputs: impl IntoIterator<Item = T>) -> bool {
        inputs.into_iter().any(|i| self.just_pressed(i))
    }

    pub fn just_released(&self, input: T) -> bool {
        self.just_released.contains(&input)
    }

    pub fn any_just_released(&self, inputs: impl IntoIterator<Item = T>) -> bool {
        inputs.into_iter().any(|i| self.just_released(i))
    }

    pub fn get_pressed(&self) -> impl ExactSizeIterator<Item = &T> {
        self.pressed.iter()
    }

    pub fn get_just_pressed(&self) -> impl ExactSizeIterator<Item = &T> {
        self.just_pressed.iter()
    }

    pub fn get_just_released(&self) -> impl ExactSizeIterator<Item = &T> {
        self.just_released.iter()
    }

    /// Forgets that `input` just went down, so nothing else acts on it.
    pub fn clear_just_pressed(&mut self, input: T) -> bool {
        self.just_pressed.remove(&input)
    }

    /// Forgets what went down and up (the buttons held stay held).
    pub fn clear(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }

    pub fn reset_all(&mut self) {
        self.pressed.clear();
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

/// A key going down or up. `key_code` is the key's place on the keyboard,
/// when the system says; `logical_key` what it means in the layout.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct KeyboardInput {
    pub key_code: Option<KeyCode>,
    pub logical_key: Key,
    pub state: ButtonState,
    pub text: Option<SmolStr>,
    pub repeat: bool,
    pub window: Entity,
}

/// How far the mouse moved since the last frame (its own units, not the
/// pointer's).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct AccumulatedMouseMotion {
    pub delta: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseScrollUnit {
    /// Notches of a wheel.
    #[default]
    Line,
    /// Pixels, from a trackpad or a smooth wheel.
    Pixel,
}

/// How far the wheel scrolled since the last frame.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct AccumulatedMouseScroll {
    pub unit: MouseScrollUnit,
    pub delta: Vec2,
}

/// One turn of the wheel.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct MouseWheel {
    pub unit: MouseScrollUnit,
    pub x: f32,
    pub y: f32,
}

/// Two fingers on a trackpad moving together (physical pixels).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct PanGesture(pub Vec2);

/// Two fingers on a trackpad spreading (positive) or pinching, as a change
/// in scale.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct PinchGesture(pub f32);

/// A finger on the screen. Positions are in logical pixels in the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    id: u64,
    start_position: Vec2,
    previous_position: Vec2,
    position: Vec2,
}

impl Touch {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn start_position(&self) -> Vec2 {
        self.start_position
    }

    /// Where it was at the start of the frame.
    pub fn previous_position(&self) -> Vec2 {
        self.previous_position
    }

    /// How far it moved this frame.
    pub fn delta(&self) -> Vec2 {
        self.position - self.previous_position
    }

    /// How far it is from where it landed.
    pub fn distance(&self) -> Vec2 {
        self.position - self.start_position
    }
}

/// The fingers on the screen, and those that landed, lifted or were taken
/// away by the system since the last frame.
#[derive(Resource, Debug, Clone, Default)]
pub struct Touches {
    pressed: HashMap<u64, Touch>,
    just_pressed: HashMap<u64, Touch>,
    just_released: HashMap<u64, Touch>,
    just_canceled: HashMap<u64, Touch>,
}

impl Touches {
    pub fn iter(&self) -> impl Iterator<Item = &Touch> {
        self.pressed.values()
    }

    pub fn get_pressed(&self, id: u64) -> Option<&Touch> {
        self.pressed.get(&id)
    }

    pub fn just_pressed(&self, id: u64) -> bool {
        self.just_pressed.contains_key(&id)
    }

    pub fn any_just_pressed(&self) -> bool {
        !self.just_pressed.is_empty()
    }

    pub fn iter_just_pressed(&self) -> impl Iterator<Item = &Touch> {
        self.just_pressed.values()
    }

    pub fn just_released(&self, id: u64) -> bool {
        self.just_released.contains_key(&id)
    }

    pub fn any_just_released(&self) -> bool {
        !self.just_released.is_empty()
    }

    pub fn iter_just_released(&self) -> impl Iterator<Item = &Touch> {
        self.just_released.values()
    }

    pub fn get_released(&self, id: u64) -> Option<&Touch> {
        self.just_released.get(&id)
    }

    pub fn just_canceled(&self, id: u64) -> bool {
        self.just_canceled.contains_key(&id)
    }

    pub fn any_just_canceled(&self) -> bool {
        !self.just_canceled.is_empty()
    }

    pub fn iter_just_canceled(&self) -> impl Iterator<Item = &Touch> {
        self.just_canceled.values()
    }

    pub fn first_pressed_position(&self) -> Option<Vec2> {
        self.pressed
            .values()
            .next()
            .or_else(|| self.just_pressed.values().next())
            .map(|t| t.position)
    }

    /// Takes in a finger's move (logical position).
    fn event(&mut self, id: u64, phase: TouchPhase, position: Vec2) {
        let new = Touch {
            id,
            start_position: position,
            previous_position: position,
            position,
        };
        match phase {
            TouchPhase::Started => {
                self.pressed.insert(id, new);
                self.just_pressed.insert(id, new);
            }
            TouchPhase::Moved => {
                if let Some(t) = self.pressed.get_mut(&id) {
                    t.position = position;
                }
            }
            TouchPhase::Ended => {
                let t = self.pressed.remove(&id).unwrap_or(new);
                self.just_released.insert(id, t);
            }
            TouchPhase::Cancelled => {
                let t = self.pressed.remove(&id).unwrap_or(new);
                self.just_canceled.insert(id, t);
            }
        }
    }
}

/// Input taken from the window since the last frame, in order.
#[derive(Debug, Clone)]
pub(crate) enum Raw {
    Key(KeyboardInput),
    MouseButton(MouseButton, ButtonState),
    MouseMotion(Vec2),
    Wheel(MouseWheel),
    Touch(u64, TouchPhase, Vec2),
    Pan(Vec2),
    Pinch(f32),
    /// The window lost the keyboard: keys held are let go.
    FocusLost,
}

pub(crate) fn setup(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<Touches>()
        .add_message::<KeyboardInput>()
        .add_message::<MouseWheel>()
        .add_message::<PanGesture>()
        .add_message::<PinchGesture>();
}

/// Takes in the input since the last frame, after forgetting what went down
/// and up in it.
pub(crate) fn take_in(world: &mut World, events: Vec<Raw>) {
    world.resource_mut::<ButtonInput<KeyCode>>().clear();
    world.resource_mut::<ButtonInput<MouseButton>>().clear();
    *world.resource_mut::<AccumulatedMouseMotion>() = default();
    *world.resource_mut::<AccumulatedMouseScroll>() = default();
    {
        let mut touches = world.resource_mut::<Touches>();
        touches.just_pressed.clear();
        touches.just_released.clear();
        touches.just_canceled.clear();
        if events.iter().any(|e| matches!(e, Raw::Touch(..))) {
            for t in touches.pressed.values_mut() {
                t.previous_position = t.position;
            }
        }
    }
    for e in events {
        match e {
            Raw::Key(k) => {
                if let Some(code) = k.key_code {
                    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
                    match k.state {
                        ButtonState::Pressed => keys.press(code),
                        ButtonState::Released => keys.release(code),
                    }
                }
                world.write_message(k);
            }
            Raw::MouseButton(b, state) => {
                let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
                match state {
                    ButtonState::Pressed => buttons.press(b),
                    ButtonState::Released => buttons.release(b),
                }
            }
            Raw::MouseMotion(d) => world.resource_mut::<AccumulatedMouseMotion>().delta += d,
            Raw::Wheel(w) => {
                let mut scroll = world.resource_mut::<AccumulatedMouseScroll>();
                if scroll.unit != w.unit {
                    scroll.delta = Vec2::ZERO;
                }
                scroll.unit = w.unit;
                scroll.delta += Vec2::new(w.x, w.y);
                world.write_message(w);
            }
            Raw::Touch(id, phase, p) => world.resource_mut::<Touches>().event(id, phase, p),
            Raw::Pan(d) => {
                world.write_message(PanGesture(d));
            }
            Raw::Pinch(d) => {
                world.write_message(PinchGesture(d));
            }
            Raw::FocusLost => world.resource_mut::<ButtonInput<KeyCode>>().release_all(),
        }
    }
}

fn default<T: Default>() -> T {
    T::default()
}
