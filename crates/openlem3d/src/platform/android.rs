//! Android: the app's directories, and the window, through Java (JNI).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use jni::objects::{JObject, JObjectArray, JString, JValue};
use jni::{JNIEnv, JavaVM};
use l3d_engine::android::android_activity::AndroidApp;
use l3d_engine::log::warn;

/// `Activity.FULLSCREEN_MODE_REQUEST_EXIT` (and `_ENTER`, one more).
const FULLSCREEN_MODE_REQUEST_EXIT: i32 = 0;
/// `WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS`.
const CUTOUT_MODE_ALWAYS: i32 = 3;
/// `WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`.
const TRANSIENT_BARS_BY_SWIPE: i32 = 2;
/// Before Android 11, `View`'s `SYSTEM_UI_FLAG_` `IMMERSIVE_STICKY`,
/// `HIDE_NAVIGATION`, `FULLSCREEN`, `LAYOUT_HIDE_NAVIGATION` and
/// `LAYOUT_FULLSCREEN`: the bars hidden until swiped in. (Not `LAYOUT_STABLE`,
/// which would keep their room in the insets.)
const IMMERSIVE_FLAGS: i32 = 0x1000 | 0x2 | 0x4 | 0x200 | 0x400;
/// Android 9, with display cutouts; Android 11, with `WindowInsetsController`
/// and `WindowInsets.Type`.
const ANDROID_9: i32 = 28;
const ANDROID_11: i32 = 30;
/// Android 15, where a layer asks for HDR headroom.
const ANDROID_15: i32 = 35;
/// How far above white the HDR layer's brightest values go, as a multiple
/// of white (the Pixel 11 Pro's display allows up to 8). The layer asks for
/// it, and Android fits it to what the display shows at the moment, rather
/// than the game following the display's ratio.
pub const HDR_HEADROOM: f32 = 8.0;
/// `ActivityInfo.COLOR_MODE_HDR`.
const COLOR_MODE_HDR: i32 = 2;

/// Whether [`full_screen`] took effect, so the game draws into cutouts.
static FULL_SCREEN: AtomicBool = AtomicBool::new(false);

pub fn app() -> &'static AndroidApp {
    l3d_engine::android::ANDROID_APP
        .get()
        .expect("android_main stores the AndroidApp before the game starts")
}

/// Runs `f` with the activity, in a frame of local references, clearing a
/// Java exception it ends in.
fn with_activity<T>(
    f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>,
) -> jni::errors::Result<T> {
    let app = app();
    // SAFETY: the pointers are the app's live JavaVM and Activity.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread_permanently()?;
    let result = env.with_local_frame(32, |env| f(env, &activity));
    if result.is_err() && env.exception_check()? {
        // Its stack trace, in the log.
        env.exception_describe()?;
        env.exception_clear()?;
    }
    result
}

/// Runs `f` with the activity on the Java main thread, which window changes
/// need.
fn on_main_thread(
    what: &'static str,
    f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<()> + Send + 'static,
) {
    app().run_on_java_main_thread(Box::new(move || {
        if let Err(e) = with_activity(f) {
            warn!("could not {what}: {e}");
        }
    }));
}

/// The first of these holding a CD image: the external files directory; the
/// external media directory, which (unlike that) file managers on the device
/// can write to; the internal `gamedata`, for where adb can't reach external
/// storage. The media directory otherwise.
pub fn data_dir() -> Option<PathBuf> {
    let app = app();
    let external = app.external_data_path();
    let media = with_activity(media_dir).unwrap_or_else(|e| {
        warn!("could not find the media directory: {e}");
        None
    });
    let internal = app.internal_data_path().map(|p| p.join("gamedata"));
    let has_cue = |dir: &PathBuf| {
        std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("cue"))
            })
        })
    };
    [external.clone(), media.clone(), internal]
        .into_iter()
        .flatten()
        .find(has_cue)
        .or(media)
        .or(external)
}

/// `getExternalMediaDirs()[0]`, which Android creates if it doesn't exist.
fn media_dir(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<Option<PathBuf>> {
    let dirs: JObjectArray = env
        .call_method(activity, "getExternalMediaDirs", "()[Ljava/io/File;", &[])?
        .l()?
        .into();
    if env.get_array_length(&dirs)? == 0 {
        return Ok(None);
    }
    let dir = env.get_object_array_element(&dirs, 0)?;
    if dir.is_null() {
        return Ok(None);
    }
    let path: JString = env
        .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
        .l()?
        .into();
    let path: String = env.get_string(&path)?.into();
    Ok(Some(PathBuf::from(path)))
}

/// `getWindow().getDecorView().getRootWindowInsets().getInsets(...)`.
pub fn insets() -> jni::errors::Result<[u32; 4]> {
    with_activity(|env, activity| {
        let window = window(env, activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
            .l()?;
        let insets = env
            .call_method(
                &decor,
                "getRootWindowInsets",
                "()Landroid/view/WindowInsets;",
                &[],
            )?
            .l()?;
        if insets.is_null() {
            // Not attached to a window yet.
            return Ok([0; 4]);
        }
        if sdk(env)? < ANDROID_11 {
            // The bars and the keyboard; no cutouts before Android 9, and
            // drawn into after.
            let mut edge = |name: &str| -> jni::errors::Result<u32> {
                Ok(env.call_method(&insets, name, "()I", &[])?.i()?.max(0) as u32)
            };
            return Ok([
                edge("getSystemWindowInsetLeft")?,
                edge("getSystemWindowInsetTop")?,
                edge("getSystemWindowInsetRight")?,
                edge("getSystemWindowInsetBottom")?,
            ]);
        }
        // The on-screen keyboard too, so that the game makes room for it.
        let mut types = inset_type(env, "systemBars")? | inset_type(env, "ime")?;
        if !FULL_SCREEN.load(Ordering::Relaxed) {
            types |= inset_type(env, "displayCutout")?;
        }
        let edges = env
            .call_method(
                &insets,
                "getInsets",
                "(I)Landroid/graphics/Insets;",
                &[JValue::Int(types)],
            )?
            .l()?;
        let mut edge = |name: &str| -> jni::errors::Result<u32> {
            Ok(env.get_field(&edges, name, "I")?.i()?.max(0) as u32)
        };
        Ok([edge("left")?, edge("top")?, edge("right")?, edge("bottom")?])
    })
}

/// See `platform::full_screen`: with the screen to itself (not in a window
/// among others, as on a desktop or in split screen), draws into cutouts and
/// hides the system bars.
pub fn full_screen() {
    on_main_thread("go full screen", |env, activity| {
        let fill = !in_multi_window(env, activity)?;
        FULL_SCREEN.store(fill, Ordering::Relaxed);
        if !fill {
            return Ok(());
        }
        if sdk(env)? < ANDROID_9 {
            return system_bars(env, activity, false);
        }
        let window = window(env, activity)?;
        let params = env
            .call_method(
                &window,
                "getAttributes",
                "()Landroid/view/WindowManager$LayoutParams;",
                &[],
            )?
            .l()?;
        env.set_field(
            &params,
            "layoutInDisplayCutoutMode",
            "I",
            JValue::Int(CUTOUT_MODE_ALWAYS),
        )?;
        env.call_method(
            &window,
            "setAttributes",
            "(Landroid/view/WindowManager$LayoutParams;)V",
            &[JValue::Object(&params)],
        )?;
        system_bars(env, activity, false)
    });
}

/// Whether [`window_fullscreen`] asked for the window to fill the screen.
static FULLSCREEN_ASKED: AtomicBool = AtomicBool::new(false);

/// See `platform::window_fullscreen`: in a desktop window (on a desktop, or a
/// phone on a monitor), asks for it to fill the screen (Android 14 and later),
/// the system bars (the taskbar, the caption bar) hidden; and back, if it was
/// asked. A phone or tablet with the screen to itself fills it anyway.
pub fn window_fullscreen(on: bool) {
    on_main_thread("change the window's full screen", move |env, activity| {
        if on && !in_multi_window(env, activity)? {
            return Ok(());
        }
        if !on && !FULLSCREEN_ASKED.load(Ordering::Relaxed) {
            return Ok(());
        }
        FULLSCREEN_ASKED.store(on, Ordering::Relaxed);
        env.call_method(
            activity,
            "requestFullscreenMode",
            "(ILandroid/os/OutcomeReceiver;)V",
            &[
                JValue::Int(FULLSCREEN_MODE_REQUEST_EXIT + on as i32),
                JValue::Object(&JObject::null()),
            ],
        )?;
        system_bars(env, activity, !on)
    });
}

/// Whether the game is in a window among others: a desktop's window, or split
/// screen.
fn in_multi_window(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<bool> {
    env.call_method(activity, "isInMultiWindowMode", "()Z", &[])?
        .z()
}

/// Shows the system bars, or hides them until swiped in from the edge.
fn system_bars(env: &mut JNIEnv, activity: &JObject, shown: bool) -> jni::errors::Result<()> {
    let window = window(env, activity)?;
    if sdk(env)? < ANDROID_11 {
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
            .l()?;
        let flags = if shown { 0 } else { IMMERSIVE_FLAGS };
        env.call_method(
            &decor,
            "setSystemUiVisibility",
            "(I)V",
            &[JValue::Int(flags)],
        )?;
        return Ok(());
    }
    let controller = env
        .call_method(
            &window,
            "getInsetsController",
            "()Landroid/view/WindowInsetsController;",
            &[],
        )?
        .l()?;
    if controller.is_null() {
        return Ok(());
    }
    env.call_method(
        &controller,
        "setSystemBarsBehavior",
        "(I)V",
        &[JValue::Int(TRANSIENT_BARS_BY_SWIPE)],
    )?;
    let bars = inset_type(env, "systemBars")?;
    let method = if shown { "show" } else { "hide" };
    env.call_method(&controller, method, "(I)V", &[JValue::Int(bars)])?;
    Ok(())
}

fn window<'a>(env: &mut JNIEnv<'a>, activity: &JObject) -> jni::errors::Result<JObject<'a>> {
    env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()
}

/// The `SurfaceView` drawn into for HDR, once added to the window (see
/// [`HdrLayer`]).
static HDR_VIEW: std::sync::Mutex<Option<jni::objects::GlobalRef>> = std::sync::Mutex::new(None);
/// Whether the view has been asked for (or found impossible).
static HDR_ASKED: AtomicBool = AtomicBool::new(false);

/// The surface of a `SurfaceView` over the window, drawn into instead of the
/// window for HDR: the system lifts the display's brightness above white
/// only for a layer that asks for it, and the window's own layer, which a
/// native activity draws into without Android's UI, never does. Holds the
/// native window on the view's current surface.
pub struct HdrLayer {
    window: std::ptr::NonNull<std::ffi::c_void>,
}

unsafe extern "C" {
    fn ANativeWindow_fromSurface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut std::ffi::c_void;
    fn ANativeWindow_release(window: *mut std::ffi::c_void);
}

impl HdrLayer {
    /// Adds the view, the first time, where HDR can be shown (Android 15
    /// and later, on a display with HDR headroom).
    pub fn prepare() -> jni::errors::Result<()> {
        if !HDR_ASKED.swap(true, Ordering::Relaxed) && with_activity(hdr_shown)? {
            on_main_thread("add the HDR view", add_hdr_view);
        }
        Ok(())
    }

    /// The view's surface, once it is ready; none until the view exists and
    /// has one, nor where HDR can't be shown.
    pub fn create() -> jni::errors::Result<Option<HdrLayer>> {
        let Some(view) = HDR_VIEW.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(None);
        };
        with_activity(|env, _| {
            let holder = env
                .call_method(
                    view.as_obj(),
                    "getHolder",
                    "()Landroid/view/SurfaceHolder;",
                    &[],
                )?
                .l()?;
            let surface = env
                .call_method(&holder, "getSurface", "()Landroid/view/Surface;", &[])?
                .l()?;
            if surface.is_null() || !env.call_method(&surface, "isValid", "()Z", &[])?.z()? {
                return Ok(None);
            }
            // SAFETY: a live JNI environment and Surface.
            let native = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
            Ok(std::ptr::NonNull::new(native).map(|window| HdrLayer { window }))
        })
    }

    /// Whether the view's surface is ready (while there is a view).
    pub fn ready() -> bool {
        let Some(view) = HDR_VIEW.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
            return false;
        };
        with_activity(|env, _| {
            let holder = env
                .call_method(
                    view.as_obj(),
                    "getHolder",
                    "()Landroid/view/SurfaceHolder;",
                    &[],
                )?
                .l()?;
            let surface = env
                .call_method(&holder, "getSurface", "()Landroid/view/Surface;", &[])?
                .l()?;
            Ok(!surface.is_null() && env.call_method(&surface, "isValid", "()Z", &[])?.z()?)
        })
        .unwrap_or(false)
    }

    /// The native window to draw into.
    pub fn window(&self) -> std::ptr::NonNull<std::ffi::c_void> {
        self.window
    }

    /// Takes the view away again, for good: its surface can't show HDR.
    pub fn give_up() {
        let Some(view) = HDR_VIEW.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        on_main_thread("remove the HDR view", move |env, _| {
            let parent = env
                .call_method(
                    view.as_obj(),
                    "getParent",
                    "()Landroid/view/ViewParent;",
                    &[],
                )?
                .l()?;
            if !parent.is_null() {
                env.call_method(
                    &parent,
                    "removeView",
                    "(Landroid/view/View;)V",
                    &[JValue::Object(view.as_obj())],
                )?;
            }
            Ok(())
        });
    }
}

impl Drop for HdrLayer {
    fn drop(&mut self) {
        // SAFETY: the reference taken by ANativeWindow_fromSurface.
        unsafe { ANativeWindow_release(self.window.as_ptr()) };
    }
}

/// Whether the display can show HDR for a layer asking for it (Android 15
/// and later).
fn hdr_shown(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<bool> {
    if sdk(env)? < ANDROID_15 {
        return Ok(false);
    }
    let display = env
        .call_method(activity, "getDisplay", "()Landroid/view/Display;", &[])?
        .l()?;
    Ok(!display.is_null()
        && env
            .call_method(&display, "isHdrSdrRatioAvailable", "()Z", &[])?
            .z()?)
}

/// Adds a `SurfaceView` filling the window, above it, asking for
/// [`HDR_HEADROOM`]; keeps it in [`HDR_VIEW`]. Input still reaches the game
/// through the window's input queue.
fn add_hdr_view(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<()> {
    let view = env.new_object(
        "android/view/SurfaceView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(activity)],
    )?;
    env.call_method(&view, "setZOrderOnTop", "(Z)V", &[JValue::Bool(1)])?;
    env.call_method(
        &view,
        "setDesiredHdrHeadroom",
        "(F)V",
        &[JValue::Float(HDR_HEADROOM)],
    )?;
    const MATCH_PARENT: i32 = -1;
    let params = env.new_object(
        "android/view/ViewGroup$LayoutParams",
        "(II)V",
        &[JValue::Int(MATCH_PARENT), JValue::Int(MATCH_PARENT)],
    )?;
    env.call_method(
        activity,
        "addContentView",
        "(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V",
        &[JValue::Object(&view), JValue::Object(&params)],
    )?;
    let window = window(env, activity)?;
    env.call_method(
        &window,
        "setColorMode",
        "(I)V",
        &[JValue::Int(COLOR_MODE_HDR)],
    )?;
    *HDR_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(env.new_global_ref(view)?);
    Ok(())
}

/// `Build.VERSION.SDK_INT`: the Android version's API level.
fn sdk(env: &mut JNIEnv) -> jni::errors::Result<i32> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()
}

/// `WindowInsets.Type.<name>()`.
fn inset_type(env: &mut JNIEnv, name: &str) -> jni::errors::Result<i32> {
    env.call_static_method("android/view/WindowInsets$Type", name, "()I", &[])?
        .i()
}
