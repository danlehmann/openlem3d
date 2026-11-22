#!/bin/sh
# C compiler and linker for Android, aarch64, 32-bit ARM or x86_64 (by the
# --target cc-rs passes, or the name it is called by; aarch64 otherwise). Uses the NDK's clang where Google
# ships one for this host (x86_64 Linux, macOS); otherwise (e.g. aarch64
# Linux) the system clang with the NDK's sysroot. The NDK's headers are for
# its own, newer clang, so then only linking uses the NDK's clang runtime.
API=26
P=$(echo "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*)
case "$0" in *++.sh) PP=++ ;; esac
# cc-rs passes an unversioned --target, which newer NDK headers reject; the
# NDK names 32-bit ARM armv7a.
TARGET=aarch64-linux-android$API
# Linking passes no --target: called as clang-armv7.sh or clang-x86_64.sh, it
# links for that.
case "$0" in
    *armv7.sh) TARGET=armv7a-linux-androideabi$API ;;
    *x86_64.sh) TARGET=x86_64-linux-android$API ;;
esac
for a in "$@"; do
    shift
    case "$a" in
        --target=aarch64-linux-android*) TARGET=aarch64-linux-android$API; a=--target=$TARGET ;;
        --target=armv7*-linux-androideabi*) TARGET=armv7a-linux-androideabi$API; a=--target=$TARGET ;;
        --target=x86_64-linux-android*) TARGET=x86_64-linux-android$API; a=--target=$TARGET ;;
    esac
    set -- "$@" "$a"
done
if "$P/bin/clang" --version >/dev/null 2>&1; then
    exec "$P/bin/$TARGET-clang$PP" "$@"
fi
LINK="-resource-dir=$(echo "$P"/lib/clang/*) -fuse-ld=lld --rtlib=compiler-rt"
for a in "$@"; do case "$a" in -c|-E|-S|-M|-MM) LINK= ;; esac; done
exec clang$PP --target=$TARGET --sysroot="$P/sysroot" $LINK "$@"
