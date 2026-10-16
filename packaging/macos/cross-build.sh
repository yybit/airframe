#!/usr/bin/env bash
# Cross-build AirFrame for macOS from Linux: zig as the C compiler, archiver and
# linker, plus a macOS SDK for the system libraries and frameworks zig does not
# ship. See README.md.
#
#   SDKROOT=~/sdk/MacOSX15.5.sdk packaging/macos/cross-build.sh
#   SDKROOT=... packaging/macos/cross-build.sh --target x86_64-apple-darwin
#
# Further arguments are passed to `cargo build`.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
target=aarch64-apple-darwin

while [[ $# -gt 0 ]]; do
    case $1 in
        --target) target=$2; shift 2 ;;
        *) break ;;
    esac
done

: "${SDKROOT:?set SDKROOT to a macOS SDK, e.g. ~/sdk/MacOSX15.5.sdk}"
[[ -d $SDKROOT/System/Library/Frameworks ]] || {
    echo "SDKROOT=$SDKROOT does not look like a macOS SDK" >&2
    exit 1
}
command -v zig >/dev/null || { echo "zig is not on PATH" >&2; exit 1; }

# zig spells the target its own way; 11.0 is rustc's minimum for darwin.
zig_triple=${target%-apple-darwin}-macos.11.0
underscored=${target//-/_}
upper=$(echo "$underscored" | tr '[:lower:]' '[:upper:]')

export SDKROOT AIRFRAME_MACOS_TRIPLE=$zig_triple MACOSX_DEPLOYMENT_TARGET=11.0
export "CC_$underscored=$root/packaging/macos/zig-cc.sh"
export "AR_$underscored=$root/packaging/macos/zig-ar.sh"
export "CARGO_TARGET_${upper}_LINKER=$root/packaging/macos/zig-cc.sh"

echo "building $target (zig target $zig_triple, SDK $SDKROOT)"
exec cargo build --manifest-path "$root/Cargo.toml" --release --target "$target" -p airframe "$@"
