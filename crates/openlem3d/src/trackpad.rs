//! Precision touchpad fingers (Windows): two or more fingers on the
//! trackpad move the camera as two fingers on a touchscreen do (see
//! `touch`). One finger is left to the pointer.
//!
//! Windows turns trackpad gestures into wheel scrolling, one axis at a time,
//! for ordinary windows; the fingers themselves come from the touchpad's raw
//! HID reports, read here on a thread of its own with a hidden window.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::ViewCamera;
use crate::menu::AppState;
use crate::touch::MOVE_PER_PX;

/// Trackpad travel across its whole width, in touchscreen pixels: the
/// fingers move the camera as far as a drag this long on the screen.
const PAD_WIDTH_PX: f32 = 1200.0;
/// How long after the fingers leave the trackpad its wheel scrolling (which
/// Windows keeps sending, coasting) is ignored.
const WHEEL_QUIET: Duration = Duration::from_millis(600);
/// A contact not reported for this long has left the trackpad.
const CONTACT_TIMEOUT: Duration = Duration::from_millis(150);

pub struct TrackpadPlugin;

impl Plugin for TrackpadPlugin {
    fn build(&self, app: &mut App) {
        start();
        app.init_resource::<Trackpad>().add_systems(
            Update,
            trackpad_camera
                .before(crate::camera_controls)
                .run_if(in_state(AppState::Playing)),
        );
    }
}

/// The fingers on the trackpad as last seen by the game.
#[derive(Resource, Default)]
pub struct Trackpad {
    /// Contact id → position, in touchscreen pixels from the pad's corner.
    previous: HashMap<u32, Vec2>,
    /// When two or more fingers were last down.
    last_gesture: Option<Instant>,
}

impl Trackpad {
    /// Whether wheel input now comes from a trackpad gesture rather than a
    /// mouse wheel: during one and shortly after.
    pub fn owns_wheel(&self) -> bool {
        self.last_gesture.is_some_and(|t| t.elapsed() < WHEEL_QUIET)
    }
}

/// Two or more fingers: dragging moves the camera sideways and up and down,
/// spreading them apart moves it forward, pinching back (as on a
/// touchscreen).
fn trackpad_camera(mut pad: ResMut<Trackpad>, mut cams: Query<&mut ViewCamera>) {
    let now: HashMap<u32, Vec2> = if game_has_focus() {
        contacts()
    } else {
        HashMap::new()
    };
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
            let spread = both[0].1.distance(both[1].1) - both[0].0.distance(both[1].0);
            for mut cam in &mut cams {
                let fwd = cam.forward();
                let right = Vec3::new(-fwd.z, 0.0, fwd.x);
                cam.pos += -right * delta.x * MOVE_PER_PX
                    + Vec3::Y * delta.y * MOVE_PER_PX
                    + fwd * spread * MOVE_PER_PX;
                cam.glide = None;
            }
        }
    }
    pad.previous = now;
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

#[cfg(not(windows))]
fn start() {}

#[cfg(not(windows))]
fn game_has_focus() -> bool {
    false
}

#[cfg(windows)]
use win::{game_has_focus, start};

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
