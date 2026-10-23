//! Platform-specific locations. Only Android differs: it has no working
//! directory, environment or home directory to look in, so the game data and
//! settings live in the app's own storage.

use std::path::PathBuf;

/// Where the CD image lives when no `--data` was given, if the platform
/// dictates it (see `docs/GROUNDRULES.md`).
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // The first of these holding an image: the external files directory;
        // the external media directory, which (unlike that) file managers on
        // the device can write to; the internal `gamedata`, for where adb
        // can't reach external storage. The media directory otherwise.
        let app = android_app();
        let external = app.external_data_path();
        let media = android_media_dir().unwrap_or_else(|e| {
            bevy::log::warn!("could not find the media directory: {e}");
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
        return [external.clone(), media.clone(), internal]
            .into_iter()
            .flatten()
            .find(has_cue)
            .or(media)
            .or(external);
    }
    #[cfg(not(target_os = "android"))]
    None
}

/// The directory to keep `openlem3d/settings.txt` in, if the platform dictates
/// it.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    return android_app().internal_data_path();
    #[cfg(not(target_os = "android"))]
    None
}

/// How far the system draws over each edge of the window, in physical pixels:
/// left, top, right, bottom. On Android these are the caption bar of a
/// desktop window, the status and navigation bars, display cutouts and the
/// on-screen keyboard.
pub fn insets() -> [u32; 4] {
    #[cfg(target_os = "android")]
    return android_insets().unwrap_or_else(|e| {
        bevy::log::warn_once!("could not read the window insets: {e}");
        [0; 4]
    });
    #[cfg(not(target_os = "android"))]
    [0; 4]
}

/// Whether typing (a level code) brings up the system's on-screen keyboard:
/// on Android, where a device often has no other.
pub const ON_SCREEN_KEYBOARD: bool = cfg!(target_os = "android");

/// Tells the user where to put the CD image.
#[cfg(target_os = "android")]
pub const DATA_HINT: &str = "Put the .cue and .bin in Android/media/org.openlem3d on the device.";
#[cfg(not(target_os = "android"))]
pub const DATA_HINT: &str =
    "Put the .cue and .bin in ./gamedata, set OPENLEM3D_DATA, or pass --data DIR.";

#[cfg(target_os = "android")]
fn android_app() -> &'static bevy::android::android_activity::AndroidApp {
    bevy::android::ANDROID_APP
        .get()
        .expect("android_main stores the AndroidApp before the game starts")
}

/// `getExternalMediaDirs()[0]`, which Android creates if it doesn't exist.
#[cfg(target_os = "android")]
fn android_media_dir() -> jni::errors::Result<Option<PathBuf>> {
    use jni::JavaVM;
    use jni::objects::{JObject, JObjectArray, JString};

    let app = android_app();
    // SAFETY: the pointers are the app's live JavaVM and Activity.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread_permanently()?;
    let result = env.with_local_frame(8, |env| {
        let dirs: JObjectArray = env
            .call_method(&activity, "getExternalMediaDirs", "()[Ljava/io/File;", &[])?
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
    });
    if result.is_err() && env.exception_check()? {
        env.exception_clear()?;
    }
    result
}

/// `getWindow().getDecorView().getRootWindowInsets().getInsets(...)`.
#[cfg(target_os = "android")]
fn android_insets() -> jni::errors::Result<[u32; 4]> {
    use jni::JavaVM;
    use jni::objects::JObject;

    let app = android_app();
    // SAFETY: the pointers are the app's live JavaVM and Activity.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread_permanently()?;
    let result = env.with_local_frame(16, |env| {
        let window = env
            .call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])?
            .l()?;
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
        let kind = |env: &mut jni::JNIEnv, name: &str| {
            env.call_static_method("android/view/WindowInsets$Type", name, "()I", &[])?
                .i()
        };
        // The on-screen keyboard too, so that the game makes room for it.
        let types = kind(env, "systemBars")? | kind(env, "displayCutout")? | kind(env, "ime")?;
        let edges = env
            .call_method(
                &insets,
                "getInsets",
                "(I)Landroid/graphics/Insets;",
                &[types.into()],
            )?
            .l()?;
        let mut edge = |name: &str| -> jni::errors::Result<u32> {
            Ok(env.get_field(&edges, name, "I")?.i()?.max(0) as u32)
        };
        Ok([edge("left")?, edge("top")?, edge("right")?, edge("bottom")?])
    });
    if result.is_err() && env.exception_check()? {
        env.exception_clear()?;
    }
    result
}
