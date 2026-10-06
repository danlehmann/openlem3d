//! Sound effects from the user's CD (`SOUND/SPOTFX`, `SOUND/VOXFX`), played
//! for the events `SOUND/SAMPLIST.TXT` names (`docs/spec/sound.md`).
//! Other modules send [`Sfx`] messages; simulation events are turned into
//! sounds here. Muting the music (M) mutes these too.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use l3d_formats::sound::Sample;
use l3d_sim::{Event, Skill};

use crate::menu::AppState;
use crate::{Data, Game};

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Sfx>()
            .init_resource::<Sounds>()
            .add_systems(Update, (sim_events, watch_play).run_if(in_state(AppState::Playing)))
            .add_systems(Update, watch_rating.run_if(in_state(AppState::Title)))
            .add_systems(OnEnter(AppState::Playing), |mut out: MessageWriter<Sfx>| {
                out.write(Sfx("VOXFX/LETSGO1"));
            })
            .add_systems(PostUpdate, play);
    }
}

/// A request to play a sample, named by its path under `SOUND/` without
/// the extension.
#[derive(Message, Clone, Copy)]
pub struct Sfx(pub &'static str);

/// The voice naming each skill, in [`Skill::ALL`] order.
pub fn skill_voice(s: Skill) -> &'static str {
    ["VOXFX/BLOCKER1", "VOXFX/TURNER1", "VOXFX/BOMBER", "VOXFX/BUILDER2", "VOXFX/BASHER2", "VOXFX/MINER2", "VOXFX/DIGGER2", "VOXFX/CLIMBER2", "VOXFX/FLOATER1"]
        [s as usize]
}

/// The voice for each rating, in `menu::RATINGS` order.
pub const RATING_VOICES: [&str; 5] = ["VOXFX/FUN2", "VOXFX/TRICKY1", "VOXFX/TAXING1", "VOXFX/MAYHEM1", "VOXFX/PRACTISE"];

/// The voice for each preset camera.
pub const CAMERA_VOICES: [&str; 4] = ["VOXFX/CAMRAONE", "VOXFX/CAMRATWO", "VOXFX/CAMRA3", "VOXFX/CAMRA4"];

/// Uploaded samples, loaded on first use (`None` when missing or empty).
#[derive(Resource, Default)]
struct Sounds(HashMap<&'static str, Option<Handle<AudioSource>>>);

/// Effect volume relative to full scale.
const VOLUME: f32 = 0.8;

fn play(
    mut commands: Commands,
    mut requests: MessageReader<Sfx>,
    mut sounds: ResMut<Sounds>,
    mut data: ResMut<Data>,
    mut sources: ResMut<Assets<AudioSource>>,
    muted: Res<crate::music::MusicMuted>,
    settings: Res<crate::settings::Settings>,
) {
    // The same sample asked for twice in a frame plays once.
    let mut played: Vec<&'static str> = Vec::new();
    for Sfx(name) in requests.read() {
        if muted.0 || settings.effects == 0 || played.contains(name) {
            continue;
        }
        played.push(name);
        let handle = sounds.0.entry(name).or_insert_with(|| {
            let sample = data.0.read(&format!("SOUND/{name}.U8")).ok().and_then(|b| Sample::parse(&b).ok())?;
            (!sample.data.is_empty()).then(|| sources.add(AudioSource { bytes: Arc::from(sample.to_wav()) }))
        });
        if let Some(h) = handle {
            commands.spawn((AudioPlayer::new(h.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOLUME * settings.effects_volume()))));
        }
    }
}

/// The skill's name when one is selected, and the camera's number when
/// another preset is chosen (not when a level sets them up).
fn watch_play(selected: Res<crate::hud::SelectedSkill>, preset: Res<crate::PresetIndex>, game: Res<Game>, mut out: MessageWriter<Sfx>) {
    if game.sim.as_ref().is_none_or(|s| s.tick == 0) {
        return;
    }
    if selected.is_changed()
        && let Some(s) = selected.0
    {
        out.write(Sfx(skill_voice(s)));
    }
    if preset.is_changed() {
        out.write(Sfx(CAMERA_VOICES[preset.0 % 4]));
    }
}

/// The rating's name when it changes on the title screen.
fn watch_rating(rating: Res<crate::menu::MenuRating>, mut out: MessageWriter<Sfx>) {
    if rating.is_changed() && !rating.is_added() {
        out.write(Sfx(RATING_VOICES[rating.0 % 5]));
    }
}

/// Sounds for what the lemmings did this frame.
fn sim_events(mut game: ResMut<Game>, mut out: MessageWriter<Sfx>, mut fanfare_played: Local<bool>) {
    let needed = game.save_requirement;
    let Some(sim) = &mut game.sim else { return };
    if sim.tick <= 1 {
        *fanfare_played = false;
    }
    for e in sim.take_events() {
        let name = match e {
            Event::Exited => "VOXFX/YIPPEE1",
            Event::Drowned => "SPOTFX/SPLASH",
            Event::Splatted => "VOXFX/WOUNDED1",
            Event::Exploded => "SPOTFX/EXPLOSN3",
            Event::Zapped => "SPOTFX/LIGHTNG",
            Event::Trapped => match sim.object_kind {
                Some(l3d_sim::objects::ObjectKind::FlameBlower) => "SPOTFX/FLAMETHR",
                Some(l3d_sim::objects::ObjectKind::Laser) => "SPOTFX/CRACKLE2",
                _ => "SPOTFX/MANTRAP1",
            },
            Event::Climbed => "SPOTFX/SUCKER2",
            Event::UmbrellaOpened => "SPOTFX/BROLLY1",
            Event::Bounced => "SPOTFX/BOING1",
            Event::Catapulted => "SPOTFX/CTAPULT1",
            Event::Teleported => "SPOTFX/TELEPORT",
            Event::Brick => "SPOTFX/BRICKS1",
            Event::BuilderDone => "VOXFX/SHRUG1",
            Event::Dug => "SPOTFX/GRAVEL3",
            Event::Bashed => "SPOTFX/HAMMER1",
            Event::Mined => "SPOTFX/PICKAXE4",
        };
        out.write(Sfx(name));
    }
    // A fanfare once enough lemmings are home.
    if sim.counts.saved >= needed && needed > 0 && !*fanfare_played {
        *fanfare_played = true;
        out.write(Sfx("SPOTFX/FANFARE"));
    }
}
