//! Clocks, as Bevy had them. [`Time<Real>`] is the wall clock;
//! [`Time<Virtual>`] is game time, which follows it but takes at most
//! `max_delta` a frame and can run faster; [`Time<Fixed>`] steps through
//! game time at a fixed rate. Plain [`Time`] is game time, or the fixed
//! clock during [`FixedUpdate`](crate::app::FixedUpdate).

use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;

/// A clock: how long the last step took and how long it has run, with
/// what drives it (`T`).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Time<T: Default + Send + Sync + 'static = ()> {
    delta: Duration,
    elapsed: Duration,
    context: T,
}

impl<T: Default + Send + Sync + 'static> Time<T> {
    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn elapsed_secs_f64(&self) -> f64 {
        self.elapsed.as_secs_f64()
    }

    fn advance(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed += delta;
    }

    fn as_generic(&self) -> Time {
        Time {
            delta: self.delta,
            elapsed: self.elapsed,
            context: (),
        }
    }
}

/// The wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct Real {
    last: Option<Instant>,
}

/// Game time.
#[derive(Debug, Clone, Copy)]
pub struct Virtual {
    max_delta: Duration,
    relative_speed: f64,
}

impl Default for Virtual {
    fn default() -> Self {
        Virtual {
            max_delta: Duration::from_millis(250),
            relative_speed: 1.0,
        }
    }
}

impl Time<Virtual> {
    /// The most game time one frame may take, however long it took.
    pub fn set_max_delta(&mut self, max: Duration) {
        self.context.max_delta = max;
    }

    /// How much faster than the wall clock game time runs.
    pub fn set_relative_speed(&mut self, speed: f32) {
        self.context.relative_speed = speed as f64;
    }
}

/// Fixed steps through game time.
#[derive(Debug, Clone, Copy)]
pub struct Fixed {
    timestep: Duration,
    /// Game time not yet stepped through.
    overstep: Duration,
}

impl Default for Fixed {
    fn default() -> Self {
        Fixed {
            timestep: Duration::from_micros(15_625),
            overstep: Duration::ZERO,
        }
    }
}

impl Time<Fixed> {
    pub fn from_hz(hz: f64) -> Self {
        Time {
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            context: Fixed {
                timestep: Duration::from_secs_f64(1.0 / hz),
                overstep: Duration::ZERO,
            },
        }
    }

    pub fn timestep(&self) -> Duration {
        self.context.timestep
    }

    /// How far game time is into the next step, as a fraction of a step.
    pub fn overstep_fraction(&self) -> f32 {
        self.context.overstep.as_secs_f32() / self.context.timestep.as_secs_f32()
    }
}

/// Moves the clocks on to now, at the start of a frame (the first frame
/// takes no time).
pub(crate) fn advance(world: &mut World) {
    let now = Instant::now();
    let mut real = world.resource_mut::<Time<Real>>();
    let delta = real.context.last.map_or(Duration::ZERO, |last| now - last);
    real.context.last = Some(now);
    real.advance(delta);
    let mut game = world.resource_mut::<Time<Virtual>>();
    let delta = delta
        .min(game.context.max_delta)
        .mul_f64(game.context.relative_speed);
    game.advance(delta);
    let generic = game.as_generic();
    *world.resource_mut::<Time>() = generic;
    world.resource_mut::<Time<Fixed>>().context.overstep += delta;
}

/// Runs `step` once for each fixed step due, with plain [`Time`] set to
/// the fixed clock meanwhile.
pub(crate) fn run_fixed(world: &mut World, mut step: impl FnMut(&mut World)) {
    let game = *world.resource::<Time>();
    loop {
        let mut fixed = world.resource_mut::<Time<Fixed>>();
        let timestep = fixed.context.timestep;
        if fixed.context.overstep < timestep {
            break;
        }
        fixed.context.overstep -= timestep;
        fixed.advance(timestep);
        let generic = fixed.as_generic();
        *world.resource_mut::<Time>() = generic;
        step(world);
    }
    *world.resource_mut::<Time>() = game;
}
