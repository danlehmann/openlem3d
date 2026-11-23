//! Choosing Enhanced's graphics for the machine (ours), until they are
//! settled (`Settings::calibrated`; a fresh start, or Default Config):
//! a first guess from the GPU's name, then a measurement in the first level
//! played. A few seconds in (past loading and shaders compiling), the
//! frames are timed for a few seconds: too slow for 60 a second, a preset
//! down (below Low, a resolution down) and measured again; with much to
//! spare, once a preset up. Then they are settled and saved.

use crate::World;
use crate::menu::AppState;
use crate::settings::Preset;

/// Seconds into a level before frames are timed, and how long they are.
const WARM_UP: f32 = 3.0;
const MEASURE: f32 = 3.0;
/// A mean frame slower than this misses 60 a second (with a little slack).
const TOO_SLOW: f32 = 1.0 / 55.0;
/// Nine frames in ten this fast: a steady 60 (or more), so worth trying a
/// preset up (a display's refresh hides any more room than that).
const ROOM_TO_SPARE: f32 = 1.0 / 58.0;

/// GPUs too slow for more than Low (by name, as the adapter gives it): small
/// Malis and PowerVRs in TV sticks and cheap boxes, and old Adrenos.
const WEAK_GPUS: [&str; 7] = [
    "Mali-G31",
    "Mali-G51",
    "Mali-G52",
    "Mali-T",
    "PowerVR Rogue GE",
    "PowerVR GE",
    "Adreno (TM) 3",
];

#[derive(Default)]
pub(crate) struct AutoDetect {
    /// The first guess has been made.
    guessed: bool,
    /// The graphics were settled since (so if they come unsettled, by
    /// Default Config, it starts afresh).
    settled: bool,
    /// Real seconds since the level began, and the frame times taken.
    since: f32,
    frames: Vec<f32>,
    /// It went up a preset once already (and won't again).
    raised: bool,
}

/// The preset to start from on a GPU named `name`.
pub(crate) fn guess(name: &str) -> Preset {
    if WEAK_GPUS.iter().any(|w| name.contains(w)) {
        Preset::Low
    } else {
        Preset::High
    }
}

pub(crate) fn update(w: &mut World) {
    if w.settings.calibrated {
        w.autodetect.settled = true;
        return;
    }
    if w.autodetect.settled {
        w.autodetect = AutoDetect::default();
    }
    if !w.settings.enhanced() {
        return;
    }
    let a = &mut w.autodetect;
    if !a.guessed {
        let Some(name) = w.renderer.adapter_name() else {
            return;
        };
        a.guessed = true;
        let preset = guess(&name);
        w.settings.graphics = preset.graphics(w.settings.graphics);
        l3d_engine::log::info!(
            "graphics: {} for {name}; measuring in the first level",
            preset.name()
        );
    }
    let a = &mut w.autodetect;
    let playing = w.state == AppState::Playing && !w.preview.active && w.confirm.question.is_none();
    if !playing {
        a.since = 0.0;
        a.frames.clear();
        return;
    }
    let dt = w.e.time.real.delta_secs();
    a.since += dt;
    if a.since < WARM_UP {
        return;
    }
    a.frames.push(dt);
    if a.since < WARM_UP + MEASURE {
        return;
    }
    let mean = a.frames.iter().sum::<f32>() / a.frames.len() as f32;
    let mut sorted = a.frames.clone();
    sorted.sort_by(f32::total_cmp);
    let p90 = sorted[sorted.len() * 9 / 10];
    a.since = 0.0;
    a.frames.clear();
    let g = w.settings.graphics;
    let lower = match g.preset {
        Preset::High => Some(Preset::Medium),
        Preset::Medium | Preset::Custom => Some(Preset::Low),
        Preset::Low => None,
    };
    let higher = match g.preset {
        Preset::Low => Some(Preset::Medium),
        Preset::Medium => Some(Preset::High),
        _ => None,
    };
    let fps = 1.0 / mean;
    if mean > TOO_SLOW {
        if let Some(p) = lower {
            l3d_engine::log::info!(
                "graphics: {fps:.0} fps at {}; trying {}",
                g.preset.name(),
                p.name()
            );
            w.settings.graphics = p.graphics(g);
            return;
        }
        // Already Low: a resolution down, while there is one.
        if (g.resolution as usize) + 1 < crate::settings::RESOLUTIONS {
            l3d_engine::log::info!("graphics: {fps:.0} fps at Low; a resolution down");
            w.settings.graphics.resolution += 1;
            w.settings.graphics.preset = Preset::Custom;
            return;
        }
    } else if p90 < ROOM_TO_SPARE
        && !w.autodetect.raised
        && let Some(p) = higher
    {
        l3d_engine::log::info!(
            "graphics: {fps:.0} fps at {}; trying {}",
            g.preset.name(),
            p.name()
        );
        w.autodetect.raised = true;
        w.settings.graphics = p.graphics(g);
        return;
    }
    // Settled, or as low as it goes (if it went up and that was too slow,
    // it is back down a preset by now, and stays).
    l3d_engine::log::info!(
        "graphics: settled on {} ({fps:.0} fps)",
        w.settings.graphics.preset.name()
    );
    w.settings.calibrated = true;
    w.settings.save();
}
