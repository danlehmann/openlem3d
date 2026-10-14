#!/bin/sh
# Builds target/android/openlem3d.apk (arm64, release code, but a debuggable
# and debug-signed APK).
#
# Needs: the Rust target aarch64-linux-android; the Android NDK
# ($ANDROID_NDK_HOME); a platform android.jar ($ANDROID_JAR); aapt, zipalign
# and apksigner (Android SDK build-tools, or the Debian packages); keytool; and
# on hosts without an NDK toolchain, clang and lld. See docs/android.md.
set -eu
cd "$(dirname "$0")/.."
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the Android NDK}"
: "${ANDROID_JAR:?set ANDROID_JAR to a platform android.jar}"
export ANDROID_NDK_HOME
HERE=$PWD/android
OUT=target/android
export CC_aarch64_linux_android="$HERE/clang.sh"
export CXX_aarch64_linux_android="$HERE/clang++.sh"
export AR_aarch64_linux_android=llvm-ar
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$HERE/clang.sh"

cargo rustc -p openlem3d --lib --release --target aarch64-linux-android --crate-type cdylib

rm -rf "$OUT/apk" && mkdir -p "$OUT/apk/lib/arm64-v8a"
llvm-strip -o "$OUT/apk/lib/arm64-v8a/libopenlem3d.so" \
    target/aarch64-linux-android/release/libopenlem3d.so
aapt package -f --debug-mode -M android/AndroidManifest.xml -I "$ANDROID_JAR" -F "$OUT/unaligned.apk" "$OUT/apk"
zipalign -f 4 "$OUT/unaligned.apk" "$OUT/aligned.apk"

KS=${ANDROID_KEYSTORE:-$HOME/.android/debug.keystore}
if [ ! -f "$KS" ]; then
    mkdir -p "$(dirname "$KS")"
    keytool -genkeypair -keystore "$KS" -storepass android -keypass android \
        -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi
apksigner sign --ks "$KS" --ks-pass pass:android --out "$OUT/openlem3d.apk" "$OUT/aligned.apk"
rm "$OUT/unaligned.apk" "$OUT/aligned.apk"
echo "$OUT/openlem3d.apk"
