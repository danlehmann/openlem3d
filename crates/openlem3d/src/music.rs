//! Level music from the CD-audio tracks of the user's disc.
//!
//! A level's theme and music index select the track. Every pair that the
//! 100 levels use was observed in the original game (`docs/spec/disc.md`).

use std::sync::Arc;

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use l3d_formats::disc::Disc;

/// The entity playing the current level's music.
#[derive(Component)]
pub struct LevelMusic;

/// Whether music is muted (M toggles).
#[derive(Resource, Default)]
pub struct MusicMuted(pub bool);

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicMuted>().add_systems(Update, toggle_mute);
    }
}

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

/// Wraps 44.1 kHz 16-bit stereo PCM in a RIFF/WAVE header.
fn wav_bytes(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // channels
    out.extend_from_slice(&44_100u32.to_le_bytes());
    out.extend_from_slice(&(44_100u32 * 4).to_le_bytes()); // byte rate
    out.extend_from_slice(&4u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Replaces the playing music with CD track `track` (1-based), looping at
/// `volume` (linear).
pub fn play_track(commands: &mut Commands, sources: &mut Assets<AudioSource>, existing: &[Entity], disc: &Disc, track: u8, volume: f32) {
    for e in existing {
        commands.entity(*e).despawn();
    }
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
    let handle = sources.add(AudioSource { bytes: Arc::from(wav_bytes(&samples)) });
    commands.spawn((AudioPlayer::new(handle), PlaybackSettings::LOOP.with_volume(Volume::Linear(volume)), LevelMusic));
    info!("playing CD track {track}");
}

/// Entities currently playing level music.
pub fn current(q: &Query<Entity, With<LevelMusic>>) -> Vec<Entity> {
    q.iter().collect()
}

pub type MusicQuery<'w, 's> = Query<'w, 's, Entity, With<LevelMusic>>;

/// The music volume: the setting, or silence while muted.
pub fn volume(muted: &MusicMuted, settings: &crate::settings::Settings) -> f32 {
    if muted.0 { 0.0 } else { settings.music_volume() }
}

/// M mutes; the playing music follows the mute and the volume setting.
fn toggle_mute(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
    mut muted: ResMut<MusicMuted>,
    mut sinks: Query<&mut AudioSink, With<LevelMusic>>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
    }
    if muted.is_changed() || settings.is_changed() {
        for mut sink in &mut sinks {
            sink.set_volume(Volume::Linear(volume(&muted, &settings)));
        }
    }
}
