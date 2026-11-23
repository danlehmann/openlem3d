//! Gamepads, through gilrs (not on Android, where none are read yet): the
//! buttons going down and up since the last frame, and where the sticks and
//! triggers are, every connected gamepad's taken together.

use crate::input::Pad;
#[cfg(not(target_os = "android"))]
use crate::input::PadButton;
#[cfg(not(target_os = "android"))]
use glam::Vec2;

/// How far a stick must be pushed before it counts.
#[cfg(not(target_os = "android"))]
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

    /// Takes in what the gamepads did since the last frame.
    pub(crate) fn take_in(&mut self, pad: &mut Pad) {
        pad.buttons.clear();
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

/// A stick's position with the dead zone taken out, the rest stretched to
/// reach 1 again.
#[cfg(not(target_os = "android"))]
fn dead(v: Vec2) -> Vec2 {
    let len = v.length();
    if len <= DEAD_ZONE {
        return Vec2::ZERO;
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
