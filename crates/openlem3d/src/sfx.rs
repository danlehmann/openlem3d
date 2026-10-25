//! Sound effects from the user's CD (`SOUND/SPOTFX`, `SOUND/VOXFX`), played
//! for the events `SOUND/SAMPLIST.TXT` names (`docs/spec/sound.md`).
//! Other modules send [`Sfx`] messages; simulation events are turned into
//! sounds here. Muting the music (M) mutes these too.

use std::collections::HashMap;

use l3d_engine::audio::Sound;
use l3d_engine::prelude::*;
use l3d_formats::sound::Sample;
use l3d_sim::{Event, Skill};

use crate::World;

/// A request to play a sample, named by its path under `SOUND/` without
/// the extension.
#[derive(Clone, Copy)]
pub struct Sfx(pub &'static str);

/// The voice naming each skill, in [`Skill::ALL`] order.
pub fn skill_voice(s: Skill) -> &'static str {
    [
        "VOXFX/BLOCKER1",
        "VOXFX/TURNER1",
        "VOXFX/BOMBER",
        "VOXFX/BUILDER2",
        "VOXFX/BASHER2",
        "VOXFX/MINER2",
        "VOXFX/DIGGER2",
        "VOXFX/CLIMBER2",
        "VOXFX/FLOATER1",
    ][s as usize]
}

/// The voice for each rating, in `menu::RATINGS` order.
pub const RATING_VOICES: [&str; 5] = [
    "VOXFX/FUN2",
    "VOXFX/TRICKY1",
    "VOXFX/TAXING1",
    "VOXFX/MAYHEM1",
    "VOXFX/PRACTISE",
];

/// The voice for each preset camera.
pub const CAMERA_VOICES: [&str; 4] = [
    "VOXFX/CAMRAONE",
    "VOXFX/CAMRATWO",
    "VOXFX/CAMRA3",
    "VOXFX/CAMRA4",
];

/// The frame's requests, the samples loaded so far (`None` when missing or
/// empty), and what the voices last named.
#[derive(Default)]
pub(crate) struct Sounds {
    pub(crate) queue: Vec<Sfx>,
    loaded: HashMap<&'static str, Option<Sound>>,
    /// The writes to the selected skill and the preset camera seen.
    skill: Option<u64>,
    preset: Option<u64>,
    rating: Option<usize>,
    fanfare_played: bool,
}

impl Sounds {
    pub(crate) fn play(&mut self, name: &'static str) {
        self.queue.push(Sfx(name));
    }
}

/// Effect volume relative to full scale.
const VOLUME: f32 = 0.8;

pub(crate) fn play(w: &mut World) {
    let sounds = &mut w.sfx;
    // The same sample asked for twice in a frame plays once.
    let mut played: Vec<&'static str> = Vec::new();
    for Sfx(name) in std::mem::take(&mut sounds.queue) {
        if w.muted.0 || w.settings.effects == 0 || played.contains(&name) {
            continue;
        }
        played.push(name);
        let data = &mut w.data;
        let sound = sounds.loaded.entry(name).or_insert_with(|| {
            let sample = data
                .0
                .read(&format!("SOUND/{name}.U8"))
                .ok()
                .and_then(|b| Sample::parse(&b).ok())?;
            (!sample.data.is_empty()).then(|| Sound {
                channels: 1,
                rate: sample.rate,
                samples: sample.data.iter().map(|&s| s as i16 * 256).collect(),
            })
        });
        if let Some(sound) = sound {
            w.e.audio.play(sound, VOLUME * w.settings.effects_volume());
        }
    }
}

/// "Let's go!" as a level starts afresh.
pub(crate) fn lets_go(w: &mut World) {
    if crate::fresh_level(w) {
        w.sfx.play("VOXFX/LETSGO1");
    }
}

/// The skill's name when one is selected, and the camera's number when
/// another preset is chosen (not when a level sets them up).
pub(crate) fn watch_play(w: &mut World) {
    let skill = w.selected.0;
    let preset = w.preset.0;
    let sounds = &mut w.sfx;
    let skill_changed = sounds.skill.replace(w.selected.1) != Some(w.selected.1);
    let preset_changed = sounds.preset.replace(w.preset.1) != Some(w.preset.1);
    if w.game.sim.as_ref().is_none_or(|s| s.tick == 0) {
        return;
    }
    if skill_changed && let Some(s) = skill {
        sounds.play(skill_voice(s));
    }
    if preset_changed {
        sounds.play(CAMERA_VOICES[preset % 4]);
    }
}

/// The rating's name when it changes on the title screen.
pub(crate) fn watch_rating(w: &mut World) {
    let rating = w.rating.0;
    if let Some(was) = w.sfx.rating.replace(rating)
        && was != rating
    {
        w.sfx.play(RATING_VOICES[rating % 5]);
    }
}

/// Sounds for what the lemmings did this frame.
pub(crate) fn sim_events(w: &mut World) {
    let needed = w.game.save_requirement;
    let Some(sim) = &mut w.game.sim else { return };
    let sounds = &mut w.sfx;
    if sim.tick <= 1 {
        sounds.fanfare_played = false;
    }
    for e in sim.take_events() {
        use l3d_sim::objects::ObjectKind;
        let name = match e {
            Event::Exited => "VOXFX/YIPPEE1",
            Event::Drowned => "VOXFX/DROWN1",
            Event::Splatted => "VOXFX/WOUNDED1",
            Event::Exploded => "SPOTFX/EXPLOSN3",
            Event::Zapped => "SPOTFX/LIGHTNG",
            Event::Electrocuted => "SPOTFX/CRACKLE2",
            Event::Screamed => "VOXFX/FALLER3",
            Event::Trapped => match sim.object_kind {
                Some(ObjectKind::FlameBlower) => "SPOTFX/FLAMETHR",
                Some(ObjectKind::Laser) => "SPOTFX/CRACKLE2",
                Some(ObjectKind::Squasher | ObjectKind::BearTrap) => "SPOTFX/MANTRAP1",
                // The weird trap is silent (recorded in the original).
                _ => continue,
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
        sounds.play(name);
    }
    // A fanfare once enough lemmings are home.
    if sim.counts.saved >= needed && needed > 0 && !sounds.fanfare_played {
        sounds.fanfare_played = true;
        sounds.play("SPOTFX/FANFARE");
    }
}
