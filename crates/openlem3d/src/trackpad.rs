//! Trackpad gestures: two or more fingers on the trackpad move the camera as
//! two fingers on a touchscreen do (see `touch`), or with Shift as an
//! action-button drag does. One finger is left to the pointer.
//!
//! Windows turns trackpad gestures into wheel scrolling, one axis at a time,
//! for ordinary windows; the fingers themselves come from the precision
//! touchpad's raw HID reports, read here on a thread of its own with a hidden
//! window. macOS hands the fingers to views that ask for them, as touches
//! on the trackpad's gesture events; its scrolling and magnifying for them
//! are dropped before winit sees them. On Android the (patched) winit
//! reports them as pan and pinch gestures.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use l3d_engine::prelude::*;

use crate::World;
use crate::touch::{Twist, TwoFingers, two_fingers};

/// Trackpad travel across its whole width, in touchscreen pixels: the
/// fingers move the camera as far as a drag this long on the screen.
#[cfg(any(windows, target_os = "macos"))]
const PAD_WIDTH_PX: f32 = 1200.0;
/// How long after the fingers leave the trackpad its wheel scrolling (which
/// Windows keeps sending, coasting) is ignored.
const WHEEL_QUIET: Duration = Duration::from_millis(600);
/// A contact not reported for this long has left the trackpad.
const CONTACT_TIMEOUT: Duration = Duration::from_millis(150);
/// A pinch gesture's change in scale, as fingers this far apart (pixels)
/// spreading or pinching.
const PINCH_SPREAD_PX: f32 = 300.0;

/// The fingers on the trackpad as last seen by the game.
#[derive(Default)]
pub struct Trackpad {
    /// Contact id → position, in touchscreen pixels from the pad's corner.
    previous: HashMap<u32, Vec2>,
    /// When two or more fingers were last down.
    last_gesture: Option<Instant>,
    /// The fingers' twist so far.
    twist: Twist,
}

impl Trackpad {
    /// Whether wheel input now comes from a trackpad gesture rather than a
    /// mouse wheel: during one and shortly after.
    pub fn owns_wheel(&self) -> bool {
        self.last_gesture.is_some_and(|t| t.elapsed() < WHEEL_QUIET)
    }
}

/// Two or more fingers move the camera as two on a touchscreen do (see
/// [`two_fingers`]), towards the pointer; with Shift, dragging turns and
/// moves forward and back.
pub(crate) fn trackpad_camera(w: &mut World) {
    let toward = ray(w);
    let turning = turning(&w.e.input.keys);
    let pad = &mut w.trackpad;
    let now: HashMap<u32, Vec2> = if game_has_focus() {
        contacts()
    } else {
        HashMap::new()
    };
    if now.len() < 2 {
        pad.twist = Twist::default();
    }
    if now.len() >= 2 {
        pad.last_gesture = Some(Instant::now());
        // Only fingers seen last frame too count, so one landing or lifting
        // doesn't jump the camera.
        let both: Vec<(Vec2, Vec2)> = now
            .iter()
            .filter_map(|(id, &p)| pad.previous.get(id).map(|&q| (q, p)))
            .collect();
        if both.len() >= 2 {
            let n = both.len() as f32;
            let delta = both.iter().map(|(q, p)| *p - *q).sum::<Vec2>() / n;
            let [(a0, a1), (b0, b1)] = [both[0], both[1]];
            let fingers = TwoFingers {
                delta,
                spread: a1.distance(b1) - a0.distance(b0),
                twist: pad.twist.turn((b0 - a0).angle_to(b1 - a1)),
                turning,
                toward,
            };
            two_fingers(&mut w.view, &fingers, &w.game);
        }
    }
    pad.previous = now;
}

/// Pan and pinch gestures, as two fingers on the trackpad.
pub(crate) fn gesture_camera(w: &mut World) {
    let input = &w.e.input;
    // Physical pixels, as winit reports them.
    let scale = w.e.window.scale_factor();
    let delta = input.pans.iter().map(|p| p.0).sum::<Vec2>() / scale;
    let spread = input.pinches.iter().map(|p| p.0).sum::<f32>() * PINCH_SPREAD_PX;
    if delta == Vec2::ZERO && spread == 0.0 {
        return;
    }
    w.trackpad.last_gesture = Some(Instant::now());
    let fingers = TwoFingers {
        delta,
        spread,
        twist: 0.0,
        turning: turning(&input.keys),
        toward: ray(w),
    };
    two_fingers(&mut w.view, &fingers, &w.game);
}

/// The way through the pointer, for pinching towards it.
fn ray(w: &World) -> Option<Vec3> {
    crate::pointer_ray(&w.e.window, &w.screen, &w.camera)
}

/// Whether two fingers dragging turn the camera, as an action-button drag
/// does, rather than move it sideways and up and down.
fn turning(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
}

/// One finger on the trackpad, as the reading thread last saw it.
#[derive(Clone, Copy)]
struct Contact {
    /// Position in touchscreen pixels from the pad's corner.
    pos: Vec2,
    seen: Instant,
}

/// The fingers on all precision touchpads, by device and contact id.
type Shared = Arc<Mutex<HashMap<(isize, u32), Contact>>>;

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(Shared::default)
}

/// The fingers now down, by contact id (of whichever trackpad has the
/// most).
fn contacts() -> HashMap<u32, Vec2> {
    let Ok(mut all) = shared().lock() else {
        return HashMap::new();
    };
    all.retain(|_, c| c.seen.elapsed() < CONTACT_TIMEOUT);
    let mut by_device: HashMap<isize, HashMap<u32, Vec2>> = HashMap::new();
    for (&(device, id), c) in all.iter() {
        by_device.entry(device).or_default().insert(id, c.pos);
    }
    by_device
        .into_values()
        .max_by_key(|m| m.len())
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn start() {}

#[cfg(not(target_os = "macos"))]
pub(crate) fn watch(_: &mut World) {}

#[cfg(not(any(windows, target_os = "macos")))]
fn game_has_focus() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) use mac::{game_has_focus, start, watch};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::cell::Cell;
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSApplication, NSEvent, NSEventMask, NSEventPhase, NSEventType, NSTouchPhase,
        NSTouchTypeMask,
    };

    pub fn start() {}

    /// The gesture events come only to the game's own windows.
    pub fn game_has_focus() -> bool {
        true
    }

    /// Asks the game's windows for the trackpad's touches (once each exists)
    /// and starts watching the events that carry them.
    pub fn watch(w: &mut World) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        PLAYING.set(w.state == crate::menu::AppState::Playing);
        for window in NSApplication::sharedApplication(mtm).windows() {
            if let Some(view) = window.contentView()
                && !view.allowedTouchTypes().contains(NSTouchTypeMask::Indirect)
            {
                view.setAllowedTouchTypes(view.allowedTouchTypes() | NSTouchTypeMask::Indirect);
            }
        }
        if !MONITORING.get() {
            MONITORING.set(true);
            let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
                let keep = see(unsafe { event.as_ref() });
                if keep {
                    event.as_ptr()
                } else {
                    std::ptr::null_mut()
                }
            });
            let mask = NSEventMask::Gesture | NSEventMask::Magnify | NSEventMask::ScrollWheel;
            // Watched for as long as the game runs.
            std::mem::forget(unsafe {
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler)
            });
        }
    }

    thread_local! {
        /// Whether the scrolling now (and its coasting after) comes from two
        /// or more fingers on a trackpad.
        static FINGERS_SCROLL: Cell<bool> = const { Cell::new(false) };
        /// Whether a level is being played: elsewhere (menus) scrolling is
        /// left as it is.
        static PLAYING: Cell<bool> = const { Cell::new(false) };
        /// Whether the events are watched yet.
        static MONITORING: Cell<bool> = const { Cell::new(false) };
    }

    /// Takes note of an event's fingers; whether it goes on to winit.
    fn see(event: &NSEvent) -> bool {
        let kind = event.r#type();
        if kind == NSEventType::ScrollWheel {
            // Scrolling by fingers (rather than a wheel) has a phase while
            // they are down, a momentum phase while it coasts after.
            if event.phase() != NSEventPhase::None {
                FINGERS_SCROLL.set(fingers_down() >= 2);
            } else if event.momentumPhase() == NSEventPhase::None {
                FINGERS_SCROLL.set(false);
            }
            return !(PLAYING.get() && FINGERS_SCROLL.get());
        }
        // The fingers come with the touch events; pinching (which has none)
        // moves the camera through them.
        if kind == NSEventType::Gesture {
            read_touches(event);
            return true;
        }
        !(PLAYING.get() && kind == NSEventType::Magnify && fingers_down() >= 2)
    }

    /// The fingers on the trackpads as of a touch event.
    fn read_touches(event: &NSEvent) {
        let Ok(mut shared) = shared().lock() else {
            return;
        };
        let now = Instant::now();
        let touches = event.touchesMatchingPhase_inView(NSTouchPhase::Touching, None);
        // Each event has all the fingers on its trackpad; those it lacks
        // have left.
        let mut devices = Vec::new();
        for touch in touches.iter() {
            let device = touch.device().map_or(0, |d| Retained::as_ptr(&d) as isize);
            if !devices.contains(&device) {
                devices.push(device);
                shared.retain(|&(d, _), _| d != device);
            }
            // A palm resting on the pad is reported too.
            if touch.isResting() {
                continue;
            }
            let identity = touch.identity();
            let hash: usize = unsafe { msg_send![&*identity, hash] };
            let (p, size) = (touch.normalizedPosition(), touch.deviceSize());
            let (w, h) = (
                PAD_WIDTH_PX,
                PAD_WIDTH_PX * (size.height / size.width.max(1.0)) as f32,
            );
            // From the pad's top left corner, as on Windows.
            let pos = Vec2::new(p.x as f32 * w, (1.0 - p.y as f32) * h);
            shared.insert((device, hash as u32), Contact { pos, seen: now });
        }
        if touches.is_empty() {
            shared.clear();
        }
    }

    /// How many fingers are on the trackpad with the most.
    fn fingers_down() -> usize {
        contacts().len()
    }
}

#[cfg(windows)]
pub(crate) use win::{game_has_focus, start};

#[cfg(windows)]
mod win {
    use super::*;
    use std::cell::RefCell;
    use windows_sys::Win32::Devices::HumanInterfaceDevice::*;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::Input::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const DIGITIZER: u16 = 0x0D;
    const TOUCH_PAD: u16 = 0x05;
    const TIP_SWITCH: u16 = 0x42;
    const CONFIDENCE: u16 = 0x47;
    const CONTACT_ID: u16 = 0x51;
    const CONTACT_COUNT: u16 = 0x54;
    const GENERIC_DESKTOP: u16 = 0x01;
    const X: u16 = 0x30;
    const Y: u16 = 0x31;

    /// Starts the thread that reads the trackpads.
    pub fn start() {
        let _ = std::thread::Builder::new()
            .name("trackpad".into())
            .spawn(|| unsafe { run() });
    }

    /// Whether a window of this process is in the foreground (the reports
    /// arrive whichever window has the focus).
    pub fn game_has_focus() -> bool {
        unsafe {
            let w = GetForegroundWindow();
            let mut pid = 0;
            !w.is_null()
                && GetWindowThreadProcessId(w, &mut pid) != 0
                && pid == GetCurrentProcessId()
        }
    }

    unsafe fn run() {
        let class: Vec<u16> = "openlem3d-trackpad\0".encode_utf16().collect();
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        unsafe {
            if RegisterClassW(&wc) == 0 {
                return;
            }
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            let device = RAWINPUTDEVICE {
                usUsagePage: DIGITIZER,
                usUsage: TOUCH_PAD,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            };
            if RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_INPUT {
            unsafe { read_input(lp as HRAWINPUT) };
        }
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    /// A trackpad's report layout: its preparsed data, and per finger (link
    /// collection) the ranges of its X and Y values.
    struct Pad {
        preparsed: Vec<u8>,
        fingers: Vec<(u16, Axis, Axis)>,
    }

    /// An axis's logical and physical extents.
    #[derive(Clone, Copy)]
    struct Axis {
        logical: (i32, i32),
        physical: (i32, i32),
    }

    impl Axis {
        /// A value as a fraction of the pad and the pad's physical length.
        fn place(&self, v: u32) -> (f32, f32) {
            let (lo, hi) = self.logical;
            let t = (v as f32 - lo as f32) / (hi - lo).max(1) as f32;
            let len = (self.physical.1 - self.physical.0) as f32;
            (t, if len > 0.0 { len } else { (hi - lo) as f32 })
        }
    }

    thread_local! {
        static PADS: RefCell<HashMap<isize, Option<Pad>>> = RefCell::new(HashMap::new());
    }

    unsafe fn pad(device: HANDLE) -> Option<Pad> {
        let mut size = 0u32;
        unsafe {
            GetRawInputDeviceInfoW(device, RIDI_PREPARSEDDATA, std::ptr::null_mut(), &mut size);
            let mut preparsed = vec![0u8; size as usize];
            if size == 0
                || GetRawInputDeviceInfoW(
                    device,
                    RIDI_PREPARSEDDATA,
                    preparsed.as_mut_ptr().cast(),
                    &mut size,
                ) == u32::MAX
            {
                return None;
            }
            let pp = preparsed.as_ptr() as PHIDP_PREPARSED_DATA;
            let mut caps: HIDP_CAPS = std::mem::zeroed();
            if HidP_GetCaps(pp, &mut caps) != HIDP_STATUS_SUCCESS {
                return None;
            }
            let mut n = caps.NumberInputValueCaps;
            let mut values = vec![std::mem::zeroed::<HIDP_VALUE_CAPS>(); n as usize];
            if HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut n, pp) != HIDP_STATUS_SUCCESS
            {
                return None;
            }
            let axis = |link: u16, usage: u16| {
                values
                    .iter()
                    .find(|v| {
                        v.UsagePage == GENERIC_DESKTOP
                            && v.LinkCollection == link
                            && if v.IsRange != 0 {
                                (v.Anonymous.Range.UsageMin..=v.Anonymous.Range.UsageMax)
                                    .contains(&usage)
                            } else {
                                v.Anonymous.NotRange.Usage == usage
                            }
                    })
                    .map(|v| Axis {
                        logical: (v.LogicalMin, v.LogicalMax),
                        physical: (v.PhysicalMin, v.PhysicalMax),
                    })
            };
            let mut links: Vec<u16> = values.iter().map(|v| v.LinkCollection).collect();
            links.sort();
            links.dedup();
            let fingers = links
                .into_iter()
                .filter_map(|l| Some((l, axis(l, X)?, axis(l, Y)?)))
                .collect::<Vec<_>>();
            (!fingers.is_empty()).then_some(Pad { preparsed, fingers })
        }
    }

    unsafe fn read_input(handle: HRAWINPUT) {
        let header = size_of::<RAWINPUTHEADER>() as u32;
        let mut size = 0u32;
        unsafe {
            GetRawInputData(handle, RID_INPUT, std::ptr::null_mut(), &mut size, header);
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            if GetRawInputData(
                handle,
                RID_INPUT,
                buf.as_mut_ptr().cast(),
                &mut size,
                header,
            ) == u32::MAX
            {
                return;
            }
            let raw = &*(buf.as_ptr() as *const RAWINPUT);
            if raw.header.dwType != RIM_TYPEHID {
                return;
            }
            let device = raw.header.hDevice;
            let hid = &raw.data.hid;
            let reports = std::slice::from_raw_parts(
                hid.bRawData.as_ptr(),
                (hid.dwSizeHid * hid.dwCount) as usize,
            );
            PADS.with_borrow_mut(|pads| {
                let Some(pad) = pads.entry(device as isize).or_insert_with(|| pad(device)) else {
                    return;
                };
                for report in reports.chunks(hid.dwSizeHid.max(1) as usize) {
                    read_report(device as isize, pad, report);
                }
            });
        }
    }

    unsafe fn read_report(device: isize, pad: &Pad, report: &[u8]) {
        let pp = pad.preparsed.as_ptr() as PHIDP_PREPARSED_DATA;
        let len = report.len() as u32;
        let value = |page: u16, link: u16, usage: u16| {
            let mut v = 0u32;
            (unsafe {
                HidP_GetUsageValue(
                    HidP_Input,
                    page,
                    link,
                    usage,
                    &mut v,
                    pp,
                    report.as_ptr(),
                    len,
                )
            } == HIDP_STATUS_SUCCESS)
                .then_some(v)
        };
        let Ok(mut shared) = shared().lock() else {
            return;
        };
        let now = Instant::now();
        // A report's first `count` fingers are real; a report continuing the
        // previous one (more fingers than fit in one) has a count of 0, and
        // its unused slots may hold anything.
        let count = value(DIGITIZER, 0, CONTACT_COUNT).unwrap_or(0) as usize;
        for (slot, &(link, x, y)) in pad.fingers.iter().enumerate() {
            if count > 0 && slot >= count {
                break;
            }
            let (Some(id), Some(vx), Some(vy)) = (
                value(DIGITIZER, link, CONTACT_ID),
                value(GENERIC_DESKTOP, link, X),
                value(GENERIC_DESKTOP, link, Y),
            ) else {
                continue;
            };
            let mut usages = [0u16; 16];
            let mut n = usages.len() as u32;
            let read = unsafe {
                HidP_GetUsages(
                    HidP_Input,
                    DIGITIZER,
                    link,
                    usages.as_mut_ptr(),
                    &mut n,
                    pp,
                    report.as_ptr() as *mut u8,
                    len,
                )
            } == HIDP_STATUS_SUCCESS;
            let on = |usage| read && usages[..n as usize].contains(&usage);
            // A palm resting on the pad is reported too, without the
            // confidence that it is a finger.
            if !(on(TIP_SWITCH) && on(CONFIDENCE)) {
                if count > 0 {
                    shared.remove(&(device, id));
                }
                continue;
            }
            let ((tx, width), (ty, height)) = (x.place(vx), y.place(vy));
            let px = PAD_WIDTH_PX;
            let pos = Vec2::new(tx * px, ty * px * height / width);
            shared.insert((device, id), Contact { pos, seen: now });
        }
    }
}
