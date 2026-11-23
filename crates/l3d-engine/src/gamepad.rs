//! Gamepads: the buttons going down and up since the last frame, and where
//! the sticks and triggers are, every connected gamepad's taken together.
//! Through gilrs, or on Android from the key codes and axes its (patched)
//! winit reports for a gamepad.

use crate::input::Pad;
use crate::input::PadButton;
#[cfg(not(target_os = "android"))]
use glam::Vec2;

/// How far a stick must be pushed before it counts.
const DEAD_ZONE: f32 = 0.18;

pub(crate) struct Pads {
    #[cfg(not(target_os = "android"))]
    gilrs: Option<gilrs::Gilrs>,
}

impl Pads {
    pub(crate) fn open() -> Self {
        #[cfg(not(target_os = "android"))]
        return Pads {
            gilrs: gilrs::Gilrs::new()
                .inspect_err(|e| log::warn!("no gamepads: {e}"))
                .ok(),
        };
        #[cfg(target_os = "android")]
        Pads {}
    }

    /// Takes in what the gamepads did since the last frame (after
    /// `input::take_in`, which clears what went down and up).
    pub(crate) fn take_in(&mut self, _pad: &mut Pad) {
        #[cfg(not(target_os = "android"))]
        let pad = _pad;
        #[cfg(not(target_os = "android"))]
        {
            let Some(gilrs) = &mut self.gilrs else { return };
            while let Some(gilrs::Event { event, .. }) = gilrs.next_event() {
                match event {
                    gilrs::EventType::ButtonPressed(b, _) => {
                        if let Some(b) = button(b) {
                            pad.buttons.press(b);
                        }
                    }
                    gilrs::EventType::ButtonReleased(b, _) => {
                        if let Some(b) = button(b) {
                            pad.buttons.release(b);
                        }
                    }
                    gilrs::EventType::Connected => {
                        log::info!("gamepad connected");
                    }
                    gilrs::EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        pad.buttons.release_all();
                    }
                    _ => {}
                }
            }
            let (mut left, mut right) = (Vec2::ZERO, Vec2::ZERO);
            let (mut lt, mut rt) = (0.0f32, 0.0f32);
            pad.connected = false;
            for (_, g) in gilrs.gamepads() {
                pad.connected = true;
                use gilrs::{Axis, Button};
                let stick = |x, y| dead(Vec2::new(g.value(x), g.value(y)));
                let l = stick(Axis::LeftStickX, Axis::LeftStickY);
                let r = stick(Axis::RightStickX, Axis::RightStickY);
                if l.length() > left.length() {
                    left = l;
                }
                if r.length() > right.length() {
                    right = r;
                }
                let trigger = |b| g.button_data(b).map_or(0.0, |d| d.value());
                lt = lt.max(trigger(Button::LeftTrigger2));
                rt = rt.max(trigger(Button::RightTrigger2));
            }
            (pad.left, pad.right) = (left, right);
            (pad.left_trigger, pad.right_trigger) = (lt, rt);
        }
    }
}

/// Android's key codes for a gamepad's buttons, as ours.
fn android_button_of(code: u32) -> Option<PadButton> {
    Some(match code {
        96 => PadButton::South,  // KEYCODE_BUTTON_A
        97 => PadButton::East,   // KEYCODE_BUTTON_B
        99 => PadButton::West,   // KEYCODE_BUTTON_X
        100 => PadButton::North, // KEYCODE_BUTTON_Y
        102 => PadButton::LeftBumper,
        103 => PadButton::RightBumper,
        104 => PadButton::LeftTrigger,
        105 => PadButton::RightTrigger,
        106 => PadButton::LeftStick,
        107 => PadButton::RightStick,
        108 => PadButton::Start,
        109 => PadButton::Select,
        19 => PadButton::Up, // KEYCODE_DPAD_*
        20 => PadButton::Down,
        21 => PadButton::Left,
        22 => PadButton::Right,
        23 => PadButton::South, // KEYCODE_DPAD_CENTER
        4 => PadButton::East,   // KEYCODE_BACK, from some pads' B
        _ => return None,
    })
}

/// Whether Android's key code `code` is a gamepad button we take.
pub(crate) fn is_android_button(code: u32) -> bool {
    android_button_of(code).is_some()
}

/// An Android gamepad's button going down or up.
pub(crate) fn android_button(pad: &mut Pad, code: u32, down: bool) {
    let Some(b) = android_button_of(code) else {
        return;
    };
    pad.connected = true;
    if down {
        pad.buttons.press(b);
    } else {
        pad.buttons.release(b);
    }
}

/// An Android gamepad's axis (`AXIS_*`) moving: the sticks (Y down there),
/// the triggers (or the gas and brake some pads report them as) and the
/// hat, as the D-pad's buttons.
pub(crate) fn android_axis(pad: &mut Pad, axis: u32, value: f32) {
    let Some(slot) = pad.axes.get_mut(axis as usize) else {
        return;
    };
    if *slot == value {
        return;
    }
    *slot = value;
    pad.connected = true;
    let a = &pad.axes;
    pad.left = dead(glam::Vec2::new(a[0], -a[1]));
    pad.right = dead(glam::Vec2::new(a[11], -a[14]));
    pad.left_trigger = a[17].max(a[23]);
    pad.right_trigger = a[18].max(a[22]);
    if axis == 15 || axis == 16 {
        let (x, y) = (a[15], a[16]);
        for (b, on) in [
            (PadButton::Left, x < -0.5),
            (PadButton::Right, x > 0.5),
            (PadButton::Up, y < -0.5),
            (PadButton::Down, y > 0.5),
        ] {
            match (on, pad.buttons.pressed(b)) {
                (true, false) => pad.buttons.press(b),
                (false, true) => pad.buttons.release(b),
                _ => {}
            }
        }
    }
}

/// A stick's position with the dead zone taken out, the rest stretched to
/// reach 1 again.
fn dead(v: glam::Vec2) -> glam::Vec2 {
    let len = v.length();
    if len <= DEAD_ZONE {
        return glam::Vec2::ZERO;
    }
    v / len * ((len - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0)
}

#[cfg(not(target_os = "android"))]
fn button(b: gilrs::Button) -> Option<PadButton> {
    use gilrs::Button as B;
    Some(match b {
        B::South => PadButton::South,
        B::East => PadButton::East,
        B::West => PadButton::West,
        B::North => PadButton::North,
        B::LeftTrigger => PadButton::LeftBumper,
        B::RightTrigger => PadButton::RightBumper,
        B::LeftTrigger2 => PadButton::LeftTrigger,
        B::RightTrigger2 => PadButton::RightTrigger,
        B::Select => PadButton::Select,
        B::Start => PadButton::Start,
        B::LeftThumb => PadButton::LeftStick,
        B::RightThumb => PadButton::RightStick,
        B::DPadUp => PadButton::Up,
        B::DPadDown => PadButton::Down,
        B::DPadLeft => PadButton::Left,
        B::DPadRight => PadButton::Right,
        _ => return None,
    })
}
