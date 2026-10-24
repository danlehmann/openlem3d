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
