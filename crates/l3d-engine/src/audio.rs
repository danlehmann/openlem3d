//! Sound: 16-bit PCM played on the system's default output. [`Audio::play`]
//! plays a sound through once; [`Audio::play_looped`] keeps one going for
//! as long as the [`Playing`] it returns is kept.

use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use rodio::{Player, Source};

/// A sound: interleaved 16-bit samples.
#[derive(Debug, Clone)]
pub struct Sound {
    pub channels: u16,
    pub rate: u32,
    pub samples: Arc<[i16]>,
}

/// The sound output, if the system has one.
#[derive(Default)]
pub struct Audio {
    output: Option<rodio::MixerDeviceSink>,
}

/// A sound playing; it stops when this goes.
pub struct Playing(Player);

impl Playing {
    /// Sets the volume (linear, 1 for as recorded).
    pub fn set_volume(&self, volume: f32) {
        self.0.set_volume(volume);
    }
}

impl Audio {
    /// Opens the default output (or, without one, plays nothing).
    pub fn open() -> Self {
        let output = rodio::DeviceSinkBuilder::open_default_sink()
            .inspect_err(|e| log::warn!("no sound output: {e}"))
            .map(|mut s| {
                s.log_on_drop(false);
                s
            })
            .ok();
        Audio { output }
    }

    fn start(&self, sound: &Sound, looped: bool, volume: f32) -> Option<Player> {
        let output = self.output.as_ref()?;
        let player = Player::connect_new(output.mixer());
        player.set_volume(volume);
        player.append(Pcm {
            sound: sound.clone(),
            next: 0,
            looped,
        });
        Some(player)
    }

    /// Plays `sound` through once at `volume` (linear).
    pub fn play(&self, sound: &Sound, volume: f32) {
        if let Some(player) = self.start(sound, false, volume) {
            player.detach();
        }
    }

    /// Plays `sound` over and over until the result is dropped.
    pub fn play_looped(&self, sound: &Sound, volume: f32) -> Option<Playing> {
        self.start(sound, true, volume).map(Playing)
    }
}

/// A [`Sound`] as rodio plays it.
struct Pcm {
    sound: Sound,
    next: usize,
    looped: bool,
}

impl Iterator for Pcm {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        let samples = &self.sound.samples;
        if self.next >= samples.len() {
            if !self.looped || samples.is_empty() {
                return None;
            }
            self.next = 0;
        }
        let s = samples[self.next];
        self.next += 1;
        Some(s as rodio::Sample / 32768.0)
    }
}

impl Source for Pcm {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(self.sound.channels.max(1)).unwrap()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(self.sound.rate.max(1)).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
