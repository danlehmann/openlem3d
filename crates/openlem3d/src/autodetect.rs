//! Choosing Enhanced's graphics for the machine (ours): a guess from the
//! GPU's name, made once it is known. Settings not yet chosen
//! (`Settings::calibrated` off: a fresh start) take it and keep it; the
//! default settings take it too (Default Config).
//!
//! A measured choice (a benchmark run outside a level, at first start) may
//! refine it later.

use crate::World;
use crate::settings::Preset;

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

/// The guess for this machine's GPU, once it is known.
static GUESS: std::sync::OnceLock<Preset> = std::sync::OnceLock::new();

/// The preset the default settings take: the guess for this GPU, or High
/// before it is known.
pub(crate) fn default_preset() -> Preset {
    GUESS.get().copied().unwrap_or(Preset::High)
}

/// The preset to start from on a GPU named `name`.
pub(crate) fn guess(name: &str) -> Preset {
    if WEAK_GPUS.iter().any(|w| name.contains(w)) {
        Preset::Low
    } else {
        Preset::High
    }
}

/// Makes the guess once the GPU is known, and gives it to settings not yet
/// chosen.
pub(crate) fn update(w: &mut World) {
    if GUESS.get().is_none() {
        let Some(name) = w.renderer.adapter_name() else {
            return;
        };
        let preset = guess(&name);
        let _ = GUESS.set(preset);
        l3d_engine::log::info!("graphics: {} suits {name}", preset.name());
    }
    if !w.settings.calibrated {
        w.settings.graphics = default_preset().graphics(w.settings.graphics);
        w.settings.calibrated = true;
        w.settings.save();
    }
}
