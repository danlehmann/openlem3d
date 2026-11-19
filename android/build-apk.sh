#!/bin/sh
# Builds target/android/openlem3d.apk (arm64, release code, but a debuggable
# and debug-signed APK).
#
# With XR=1, builds the VR version for Meta Quest headsets instead: the `xr`
# feature, the Quest manifest, and the Khronos OpenXR loader (downloaded from
# Maven Central once, into target/android/, and checked).
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

MANIFEST=android/AndroidManifest.xml
FEATURES=
if [ "${XR:-0}" = 1 ]; then
    MANIFEST=android/AndroidManifest-quest.xml
    FEATURES="--features xr"
fi

# shellcheck disable=SC2086
cargo rustc -p openlem3d --lib --release --target aarch64-linux-android --crate-type cdylib $FEATURES

rm -rf "$OUT/apk" && mkdir -p "$OUT/apk/lib/arm64-v8a"
llvm-strip -o "$OUT/apk/lib/arm64-v8a/libopenlem3d.so" \
    target/aarch64-linux-android/release/libopenlem3d.so
if [ "${XR:-0}" = 1 ]; then
    LOADER_VERSION=1.1.65
    LOADER_SHA256=a298c326085b09181c85235fb04d37510939bd30938ed4322c093c09780e8643
    AAR="$OUT/openxr_loader_for_android-$LOADER_VERSION.aar"
    if [ ! -f "$AAR" ]; then
        curl -sfL -o "$AAR.part" "https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/$LOADER_VERSION/openxr_loader_for_android-$LOADER_VERSION.aar"
        mv "$AAR.part" "$AAR"
    fi
    echo "$LOADER_SHA256  $AAR" | shasum -a 256 -c - >/dev/null
    unzip -p "$AAR" jni/arm64-v8a/libopenxr_loader.so > "$OUT/apk/lib/arm64-v8a/libopenxr_loader.so"
fi
# aapt takes a manifest only by that name.
cp "$MANIFEST" "$OUT/AndroidManifest.xml"
aapt package -f --debug-mode -M "$OUT/AndroidManifest.xml" -I "$ANDROID_JAR" -F "$OUT/unaligned.apk" "$OUT/apk"
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
