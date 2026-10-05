//! Level music from the CD-audio tracks of the user's disc.
//!
//! The mapping from a level's theme and music index to a track is
//! PROVISIONAL (see `docs/spec/disc.md`); it still has to be observed in the
//! original game.

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

/// Provisional track choice: two tracks per theme (1–10) starting at track
/// 2, picking the second for odd music indices.
pub fn track_for(theme: u8, music: u8) -> u8 {
    let theme = theme.clamp(1, 10);
    2 + (theme - 1) * 2 + (music & 1)
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

/// Replaces the playing music with CD track `track` (1-based), looping.
pub fn play_track(commands: &mut Commands, sources: &mut Assets<AudioSource>, existing: &[Entity], disc: &Disc, track: u8, muted: bool) {
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
    let volume = if muted { Volume::SILENT } else { Volume::Linear(0.6) };
    commands.spawn((AudioPlayer::new(handle), PlaybackSettings::LOOP.with_volume(volume), LevelMusic));
    info!("playing CD track {track}");
}

/// Entities currently playing level music.
pub fn current(q: &Query<Entity, With<LevelMusic>>) -> Vec<Entity> {
    q.iter().collect()
}

pub type MusicQuery<'w, 's> = Query<'w, 's, Entity, With<LevelMusic>>;

fn toggle_mute(
    keys: Res<ButtonInput<KeyCode>>,
    mut muted: ResMut<MusicMuted>,
    mut sinks: Query<&mut AudioSink, With<LevelMusic>>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
        for mut sink in &mut sinks {
            sink.set_volume(if muted.0 { Volume::SILENT } else { Volume::Linear(0.6) });
        }
    }
}
