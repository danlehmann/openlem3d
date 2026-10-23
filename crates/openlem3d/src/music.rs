//! Level music from the CD-audio tracks of the user's disc.
//!
//! A level's theme and music index select the track. Every pair that the
//! 100 levels use was observed in the original game (`docs/spec/disc.md`).

use l3d_engine::audio::{Audio, Playing, Sound};
use l3d_engine::prelude::*;
use l3d_formats::disc::Disc;

use crate::World;
use crate::menu::AppState;

/// The current level's music, while it plays, and the volume last given it.
#[derive(Default)]
pub struct LevelMusic {
    playing: Option<Playing>,
    volume: f32,
}

/// Whether music is muted (M toggles).
#[derive(Default)]
pub struct MusicMuted(pub bool);

/// CD tracks observed in the original for (theme, music index) pairs
/// (`docs/spec/disc.md`, "Level music").
const OBSERVED_TRACKS: &[((u8, u8), u8)] = &[
    ((1, 0), 13),
    ((1, 1), 22),
    ((1, 2), 13),
    ((2, 0), 8),
    ((2, 1), 19),
    ((2, 2), 8),
    ((3, 0), 4),
    ((3, 1), 15),
    ((3, 2), 4),
    ((4, 0), 14),
    ((4, 1), 23),
    ((4, 2), 14),
    ((5, 0), 10),
    ((5, 1), 21),
    ((5, 2), 10),
    ((6, 0), 7),
    ((6, 1), 18),
    ((6, 2), 7),
    ((7, 0), 5),
    ((7, 1), 16),
    ((7, 2), 5),
    ((8, 0), 9),
    ((8, 1), 20),
    ((8, 2), 9),
    ((9, 0), 6),
    ((9, 1), 17),
    ((9, 2), 6),
    ((10, 0), 11),
    ((10, 1), 12),
    ((10, 2), 11),
];

/// The CD track for a level's theme and music index: the observed track,
/// or a placeholder for pairs that no original level uses.
pub fn track_for(theme: u8, music: u8) -> u8 {
    if let Some(&(_, track)) = OBSERVED_TRACKS.iter().find(|(k, _)| *k == (theme, music)) {
        return track;
    }
    // Placeholder for pairs outside the observed table.
    let theme = theme.clamp(1, 10);
    2 + ((theme - 1) * 2 + (music & 1)) % 23
}

/// Replaces the playing music with CD track `track` (1-based), looping at
/// `volume` (linear).
pub fn play_track(music: &mut LevelMusic, audio: &Audio, disc: &Disc, track: u8, volume: f32) {
    music.playing = None;
    let Some(t) = disc.tracks.iter().find(|t| t.number == track) else {
        warn!("no CD track {track}");
        return;
    };
    let samples = match disc.read_audio(t) {
        Ok(s) => s,
        Err(e) => {
            warn!("CD track {track}: {e}");
            return;
        }
    };
    let sound = Sound {
        channels: 2,
        rate: 44_100,
        samples: samples.into(),
    };
    music.playing = audio.play_looped(&sound, volume);
    music.volume = volume;
    info!("playing CD track {track}");
}

/// The music volume: the setting, or silence while muted.
pub fn volume(muted: &MusicMuted, settings: &crate::settings::Settings) -> f32 {
    if muted.0 {
        0.0
    } else {
        settings.music_volume()
    }
}

/// M mutes; the playing music follows the mute and the volume setting.
pub(crate) fn toggle_mute(w: &mut World) {
    if w.e.input.keys.just_pressed(KeyCode::KeyM) {
        w.muted.0 = !w.muted.0;
    }
    let v = volume(&w.muted, &w.settings);
    let music = &mut w.music;
    if v != music.volume {
        music.volume = v;
        if let Some(playing) = &music.playing {
            playing.set_volume(v);
        }
    }
}

/// Music plays only in levels, not on the briefing or in the menus
/// (verified); the options screen opened from a level keeps it.
pub(crate) fn enter(w: &mut World, state: AppState) {
    use AppState::*;
    if matches!(state, Title | Code | Menu | Practice | Briefing | Results) {
        w.music.playing = None;
    }
}
