#!/usr/bin/env bash
# cc / linker for a macOS target when cross-building from Linux with zig.
# Used as CC_<triple>, AR_<triple> (see zig-ar.sh) and
# CARGO_TARGET_<TRIPLE>_LINKER. A native macOS build uses Xcode's toolchain
# instead, so this is only reached from a cross build.
#
# Requires a macOS SDK in SDKROOT: zig's own Darwin libc is a bare
# libSystem.tbd, with no libobjc and no frameworks for the GUI stack to link
# against. See README.md.
#
# `--target=` is dropped: cc-rs spells Apple's arch `arm64-apple-macosx`, which
# zig's target parser rejects, and this wrapper is the single place that decides
# what we are building for.
args=()
for arg in "$@"; do
    case "$arg" in
        --target=*) ;;
        *) args+=("$arg") ;;
    esac
done

sysroot=()
if [[ -n ${SDKROOT:-} ]]; then
    # -isysroot is for the C compiler's headers; zig's own linker ignores it
    # when resolving -l/-framework, so point it at the SDK explicitly.
    sysroot=(
        -isysroot "$SDKROOT"
        "-L$SDKROOT/usr/lib"
        "-F$SDKROOT/System/Library/Frameworks"
    )
fi

exec zig cc -target "$AIRFRAME_MACOS_TRIPLE" "${sysroot[@]}" "${args[@]}"
