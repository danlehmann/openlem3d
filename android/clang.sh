#!/bin/sh
# C compiler and linker for aarch64 Android. Uses the NDK's clang where Google
# ships one for this host (x86_64 Linux, macOS); otherwise (e.g. aarch64
# Linux) the system clang with the NDK's sysroot. The NDK's headers are for
# its own, newer clang, so then only linking uses the NDK's clang runtime.
API=26
P=$(echo "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*)
case "$0" in *++.sh) PP=++ ;; esac
if "$P/bin/clang" --version >/dev/null 2>&1; then
    exec "$P/bin/aarch64-linux-android$API-clang$PP" "$@"
fi
LINK="-resource-dir=$(echo "$P"/lib/clang/*) -fuse-ld=lld --rtlib=compiler-rt"
for a in "$@"; do case "$a" in -c|-E|-S|-M|-MM) LINK= ;; esac; done
exec clang$PP --target=aarch64-linux-android$API --sysroot="$P/sysroot" $LINK "$@"
