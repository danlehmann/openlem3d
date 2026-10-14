# Android

The game builds as a code-free APK: Android's `NativeActivity` loads the game
library (`libopenlem3d.so`) directly, so there is no Java and no Gradle. Only
arm64 devices are supported.

## Installing the game data

The game looks for the `.cue` and `.bin` in the app's external files directory,
`Android/data/org.openlem3d/files/`, or failing that in its internal storage,
`files/gamedata/`. Install and start the app once first, so the directories
exist.

Android 11 and later hide the external directory from most file managers, so
the simplest way is adb:

    adb push lemmings3d.cue lemmings3d.bin /sdcard/Android/data/org.openlem3d/files/

That fails where the device's interactive user isn't user 0 (e.g. Android
desktops, which run in headless-system-user mode): adb's shell can't reach the
other user's storage. Copy into internal storage through `run-as` instead
(which works because the APK is debuggable), substituting the user number:

    adb push lemmings3d.cue lemmings3d.bin /data/local/tmp/
    adb shell run-as org.openlem3d --user 10 sh -c \
        "'mkdir -p files/gamedata && cp /data/local/tmp/lemmings3d.* files/gamedata/'"
    adb shell rm /data/local/tmp/lemmings3d.cue /data/local/tmp/lemmings3d.bin

The game's output (including why it couldn't open the data) goes to
`adb logcat`, tagged `RustStdoutStderr`. Settings are kept in internal storage.

## Building

    ANDROID_NDK_HOME=... ANDROID_JAR=.../android.jar android/build-apk.sh

builds `target/android/openlem3d.apk`: optimised code, but a debuggable APK
signed with a debug key (which the script creates in `~/.android/debug.keystore`
if it is missing). It needs:

- the Rust target: `rustup target add aarch64-linux-android`;
- the [Android NDK](https://developer.android.com/ndk/downloads), unpacked
  anywhere (`ANDROID_NDK_HOME`);
- a platform `android.jar` (`ANDROID_JAR`), from an SDK platform package in
  Google's SDK repository (or an installed SDK's `platforms/android-*/`);
- `aapt`, `zipalign` and `apksigner`, from the SDK build-tools or, on Debian
  and Ubuntu, the packages of the same names; `keytool` (any JDK);
- `llvm-strip` and `llvm-ar` (LLVM).

### On arm64 Linux

Google ships the NDK's compilers only for x86_64 Linux, macOS and Windows.
Elsewhere `android/clang.sh` uses the system clang (and lld) with the NDK's
sysroot instead, so install `clang lld llvm` too. Debian's packaged `aapt`,
`zipalign` and `apksigner` are built for arm64. Its `aapt` is from Android 14
and can't read newer platform jars, so use `platform-34-ext7_r03.zip`
(<https://dl.google.com/android/repository/platform-34-ext7_r03.zip>).

## Installing the APK

    adb install -r target/android/openlem3d.apk

or copy it to the device and open it there. From the Android Linux terminal,
copying it to `/mnt/shared/LinuxShare` puts it in the device's
Download/LinuxShare folder.
