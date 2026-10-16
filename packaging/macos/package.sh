#!/usr/bin/env bash
# Assemble AirFrame.app and a zip of it:
#
#   dist/AirFrame-<version>-macos-<arch>/AirFrame.app
#   dist/AirFrame-<version>-macos-<arch>.zip
#
# Usage: packaging/macos/package.sh [--arch aarch64|x86_64|universal] [--binary PATH]
#                                   [--out DIR] [--skip-build]
#
# Building is native macOS only (`cargo build --release`, optionally with
# `--target`) — see packaging/macos/README.md for why cross-building from Linux
# does not work. Signing is ad-hoc (`codesign --force -s -`), which is what a
# local build needs on Apple Silicon; a distributed build should be signed with
# a Developer ID and notarised instead.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# Default to this machine's architecture, so `package.sh` on an Intel Mac does
# not quietly produce an arm64 bundle that cannot run there.
case $(uname -m) in
    arm64 | aarch64) arch=aarch64 ;;
    x86_64) arch=x86_64 ;;
    *) arch=aarch64 ;;
esac
binary=
out=$root/dist
build=1

while [[ $# -gt 0 ]]; do
    case $1 in
        --arch) arch=$2; shift 2 ;;
        --binary) binary=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        --skip-build) build=0; shift ;;
        -h | --help) sed -n '2,15p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

case $arch in
    aarch64) triple=aarch64-apple-darwin ;;
    x86_64) triple=x86_64-apple-darwin ;;
    universal) triple=universal ;;
    *) echo "--arch must be aarch64, x86_64 or universal" >&2; exit 2 ;;
esac

# Absolute, because the archiver step zips from inside the staging directory.
mkdir -p "$out"
out=$(cd "$out" && pwd)

version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
short_version=${version%%-*}
sha=$(git -C "$root" rev-parse --short HEAD 2>/dev/null || echo unknown)
stage=$out/AirFrame-$version-macos-$arch
app=$stage/AirFrame.app

if [[ -z $binary ]]; then
    if ((build)); then
        if [[ $triple == universal ]]; then
            for t in aarch64-apple-darwin x86_64-apple-darwin; do
                cargo build --manifest-path "$root/Cargo.toml" --release --target "$t" -p airframe
            done
        else
            cargo build --manifest-path "$root/Cargo.toml" --release --target "$triple" -p airframe
        fi
    fi
    binary=$root/target/$triple/release/airframe
fi
if [[ $triple != universal ]]; then
    [[ -f $binary ]] || { echo "no binary at $binary" >&2; exit 1; }
fi

rm -rf "$stage"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

if [[ $triple == universal ]]; then
    lipo -create \
        "$root/target/aarch64-apple-darwin/release/airframe" \
        "$root/target/x86_64-apple-darwin/release/airframe" \
        -output "$app/Contents/MacOS/airframe" 2>/dev/null ||
        { echo "universal needs both target builds and lipo (macOS only)" >&2; exit 1; }
else
    cp "$binary" "$app/Contents/MacOS/airframe"
fi
chmod +x "$app/Contents/MacOS/airframe"

# The icon is rasterised at package time so the bundle and the window icon
# share one definition; see crates/ui/src/app_icon.rs.
icns=$stage/AirFrame.icns
cargo run --manifest-path "$root/Cargo.toml" --release -p airframe \
    --example export_app_icon -- "$icns" ||
    echo "warning: no icon; carrying on without one" >&2
[[ -f $icns ]] && cp "$icns" "$app/Contents/Resources/AirFrame.icns"

sed -e "s/@VERSION@/$version/g" \
    -e "s/@SHORT_VERSION@/$short_version/g" \
    -e "s/@BUILD_SHA@/$sha/g" \
    "$root/packaging/macos/Info.plist.in" >"$app/Contents/Info.plist"

# Validate the plist if this is macOS.
command -v plutil >/dev/null && plutil -lint "$app/Contents/Info.plist"

if command -v codesign >/dev/null; then
    codesign --force --sign - "$app"
    codesign --verify --deep --strict "$app"
    echo "ad-hoc signed: $app"
else
    echo "note: no codesign here, so $app stays unsigned;" >&2
    echo "      on the target Mac run: codesign --force --sign - AirFrame.app" >&2
fi

if command -v ditto >/dev/null; then
    (cd "$stage" && ditto -c -k --keepParent AirFrame.app "$stage.zip")
elif command -v zip >/dev/null; then
    (cd "$stage" && zip -qr "$stage.zip" AirFrame.app)
elif command -v bsdtar >/dev/null; then
    (cd "$stage" && bsdtar -a -cf "$stage.zip" AirFrame.app)
else
    echo "no archiver here: need one of ditto, zip or bsdtar" >&2
    exit 1
fi
echo "packaged $stage.zip"
