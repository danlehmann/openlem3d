#!/bin/sh
# Builds target/android/openlem3d.apk (release code, but a debuggable and
# debug-signed APK) for 64-bit and 32-bit ARM and for x86_64: some devices run
# only 32-bit apps (the Chromecast with Google TV); x86_64 is for Chromebooks
# and the emulator. ABIS limits the ABIs built, of arm64-v8a, armeabi-v7a and
# x86_64 (ABIS=arm64-v8a builds faster while developing; the VR build is
# arm64 only).
#
# With XR=1, builds the VR version for Meta Quest headsets instead: the `xr`
# feature, the Quest manifest, and the Khronos OpenXR loader (downloaded from
# Maven Central once, into target/android/, and checked).
#
# Needs: the Rust targets aarch64-linux-android, armv7-linux-androideabi and
# x86_64-linux-android (those of the ABIs built); the Android NDK
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
for t in aarch64_linux_android armv7_linux_androideabi x86_64_linux_android; do
    export "CC_$t=$HERE/clang.sh" "CXX_$t=$HERE/clang++.sh" "AR_$t=llvm-ar"
done
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$HERE/clang.sh"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$HERE/clang-armv7.sh"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$HERE/clang-x86_64.sh"
ABIS=${ABIS:-arm64-v8a armeabi-v7a x86_64}

MANIFEST=android/AndroidManifest.xml
FEATURES=
if [ "${XR:-0}" = 1 ]; then
    MANIFEST=android/AndroidManifest-quest.xml
    FEATURES="--features xr"
    ABIS=arm64-v8a
fi

rm -rf "$OUT/apk"
for abi in $ABIS; do
    case "$abi" in
        arm64-v8a) target=aarch64-linux-android ;;
        armeabi-v7a) target=armv7-linux-androideabi ;;
        x86_64) target=x86_64-linux-android ;;
        *) echo "unknown ABI $abi" >&2; exit 1 ;;
    esac
    # shellcheck disable=SC2086
    cargo rustc -p openlem3d --lib --release --target $target --crate-type cdylib $FEATURES
    mkdir -p "$OUT/apk/lib/$abi"
    llvm-strip -o "$OUT/apk/lib/$abi/libopenlem3d.so" "target/$target/release/libopenlem3d.so"
done
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
