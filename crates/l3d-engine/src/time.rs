//! Clocks. [`Time<Real>`] is the wall clock; [`Time<Virtual>`] is game
//! time, which follows it but takes at most `max_delta` of it a frame and
//! can run faster; [`Time<Fixed>`] steps through game time at a fixed rate,
//! as the game asks it to ([`Clocks::fixed_step`]).

use std::time::{Duration, Instant};

/// A clock: how long the last step took and how long it has run, with
/// what drives it (`T`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Time<T: Default = ()> {
    delta: Duration,
    elapsed: Duration,
    context: T,
}

impl<T: Default> Time<T> {
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

    fn advance(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed += delta;
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
    /// The most wall-clock time one frame may count, however long it took.
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
    pub fn set_hz(&mut self, hz: f64) {
        self.context.timestep = Duration::from_secs_f64(1.0 / hz);
    }

    pub fn timestep(&self) -> Duration {
        self.context.timestep
    }

    /// How far game time is into the next step, as a fraction of a step.
    pub fn overstep_fraction(&self) -> f32 {
        self.context.overstep.as_secs_f32() / self.context.timestep.as_secs_f32()
    }
}

/// The three clocks.
#[derive(Debug, Clone, Copy, Default)]
pub struct Clocks {
    pub real: Time<Real>,
    pub game: Time<Virtual>,
    pub fixed: Time<Fixed>,
}

impl Clocks {
    /// Moves the clocks on to now, at the start of a frame (the first frame
    /// takes no time).
    pub(crate) fn advance(&mut self) {
        let now = Instant::now();
        let real = self
            .real
            .context
            .last
            .map_or(Duration::ZERO, |last| now - last);
        self.real.context.last = Some(now);
        self.real.advance(real);
        let game = real
            .min(self.game.context.max_delta)
            .mul_f64(self.game.context.relative_speed);
        self.game.advance(game);
        self.fixed.context.overstep += game;
    }

    /// Takes the next fixed step if one is due: call until it says no.
    pub fn fixed_step(&mut self) -> bool {
        let f = &mut self.fixed;
        if f.context.overstep < f.context.timestep {
            return false;
        }
        f.context.overstep -= f.context.timestep;
        let step = f.context.timestep;
        f.advance(step);
        true
    }
}
