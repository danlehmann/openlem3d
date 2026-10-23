//! The app: a world, the schedules a frame runs, and plugins that fill
//! them. A frame runs, in order: [`First`], [`PreUpdate`], the state
//! transitions, [`FixedUpdate`] as many times as fixed steps are due,
//! [`Update`], [`PostUpdate`] and [`Last`]. The very first frame runs the
//! state transitions (entering the initial states) and then [`Startup`]
//! before it, as Bevy did.

use bevy_ecs::message::{Message, MessageRegistry, message_update_system};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel};
use bevy_ecs::system::ScheduleSystem;

use crate::state::{self, States};
use crate::time::{Fixed, Real, Time, Virtual};

/// Runs once, before the first frame.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Startup;
/// The start of each frame: messages and the screen's measurements.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct First;
/// After the frame's input has been taken in.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreUpdate;
/// The fixed-rate steps (the simulation's ticks).
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedUpdate;
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Update;
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostUpdate;
/// The end of each frame: drawing.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Last;
/// Where the states' changes are applied (between [`PreUpdate`] and the
/// fixed steps).
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateTransition;

/// Asks the app to quit, as a message.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppExit {
    Success,
}

pub trait Plugin {
    fn build(&self, app: &mut App);
}

/// A plugin or a tuple of them.
pub trait Plugins {
    fn add_to(self, app: &mut App);
}

impl<P: Plugin> Plugins for P {
    fn add_to(self, app: &mut App) {
        self.build(app);
    }
}

macro_rules! plugin_tuples {
    ($($p:ident),*) => {
        impl<$($p: Plugins),*> Plugins for ($($p,)*) {
            #[allow(non_snake_case)]
            fn add_to(self, app: &mut App) {
                let ($($p,)*) = self;
                $($p.add_to(app);)*
            }
        }
    };
}
plugin_tuples!(A);
plugin_tuples!(A, B);
plugin_tuples!(A, B, C);
plugin_tuples!(A, B, C, D);
plugin_tuples!(A, B, C, D, E);
plugin_tuples!(A, B, C, D, E, F);
plugin_tuples!(A, B, C, D, E, F, G);
plugin_tuples!(A, B, C, D, E, F, G, H);
plugin_tuples!(A, B, C, D, E, F, G, H, I);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K, L);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K, L, M);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
plugin_tuples!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub struct App {
    world: World,
    started: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        crate::logging::init();
        let mut world = World::new();
        for label in [
            Startup.intern(),
            First.intern(),
            PreUpdate.intern(),
            StateTransition.intern(),
            FixedUpdate.intern(),
            Update.intern(),
            PostUpdate.intern(),
            Last.intern(),
        ] {
            world.add_schedule(Schedule::new(label));
        }
        world.init_resource::<Time>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Time<Virtual>>();
        world.init_resource::<Time<Fixed>>();
        let mut app = App {
            world,
            started: false,
        };
        app.add_message::<AppExit>()
            .add_systems(First, message_update_system);
        crate::input::setup(&mut app);
        crate::window::setup(&mut app);
        crate::audio::setup(&mut app);
        app.init_asset::<crate::image::Image>();
        app
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn add_plugins(&mut self, plugins: impl Plugins) -> &mut Self {
        plugins.add_to(self);
        self
    }

    pub fn add_systems<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.world
            .resource_mut::<Schedules>()
            .add_systems(schedule, systems);
        self
    }

    pub fn configure_sets<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        sets: impl IntoScheduleConfigs<bevy_ecs::schedule::InternedSystemSet, M>,
    ) -> &mut Self {
        self.world
            .resource_mut::<Schedules>()
            .configure_sets(schedule, sets);
        self
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        if !self.world.contains_resource::<Messages<M>>() {
            MessageRegistry::register_message::<M>(&mut self.world);
        }
        self
    }

    /// Adds state `S`, starting at its default, unless it is there already.
    pub fn init_state<S: States + Default>(&mut self) -> &mut Self {
        if !self.world.contains_resource::<state::State<S>>() {
            self.insert_state(S::default());
        }
        self
    }

    /// Adds state `S` starting at `initial`, or makes `initial` its starting
    /// value.
    pub fn insert_state<S: States>(&mut self, initial: S) -> &mut Self {
        let new = !self.world.contains_resource::<state::State<S>>();
        state::insert(&mut self.world, initial);
        if new {
            self.add_systems(StateTransition, state::apply::<S>);
        }
        self
    }

    /// Runs the app: opens the window and runs frames until it quits.
    pub fn run(&mut self) {
        let app = std::mem::take(self);
        crate::runner::run(app);
    }

    /// Runs one frame (the first also runs the start-up schedules).
    pub(crate) fn update(&mut self) {
        let world = &mut self.world;
        if !self.started {
            self.started = true;
            world.run_schedule(StateTransition);
            world.run_schedule(Startup);
        }
        crate::time::advance(world);
        world.run_schedule(First);
        world.run_schedule(PreUpdate);
        world.run_schedule(StateTransition);
        crate::time::run_fixed(world, |world| world.run_schedule(FixedUpdate));
        world.run_schedule(Update);
        world.run_schedule(PostUpdate);
        world.run_schedule(Last);
        world.clear_trackers();
    }

    /// Whether something asked the app to quit since the last frame.
    pub(crate) fn should_exit(&self) -> bool {
        self.world
            .get_resource::<Messages<AppExit>>()
            .is_some_and(|m| !m.is_empty())
    }
}
