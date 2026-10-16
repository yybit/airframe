#!/usr/bin/env bash
# Build and package AirFrame for Linux (<arch> is x86_64 or aarch64):
#
#   $DIST/airframe-<version>-linux-<arch>.tar.gz     plain FHS-style tree (bin/, share/)
#   $DIST/airframe-<version>-linux-<arch>.deb        Debian, Ubuntu, Mint, Pop!_OS, ...
#   $DIST/airframe-<version>-linux-<arch>.rpm        Fedora, openSUSE, RHEL, ...
#   $DIST/AirFrame-<version>-linux-<arch>.AppImage   any distro with glibc >= build host's
#
# Usage: packaging/linux/package.sh [--arch x86_64|aarch64] [--skip-build]
#                                   [--formats "tar deb rpm appimage"] [--out DIR]
#
# Needs: cargo. nfpm (deb/rpm) and appimagetool (AppImage) are downloaded at
# pinned versions into $out/.tools unless already found in PATH. Build on the
# oldest distro you want to support (CI: Ubuntu 22.04, glibc 2.35); the
# binaries run on newer ones.
set -euo pipefail

NFPM_VERSION=2.35.0
APPIMAGETOOL_URL=https://github.com/AppImage/AppImageKit/releases/download/continuous

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# Default to this machine's architecture.
case $(uname -m) in
    x86_64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac
formats="tar deb rpm appimage"
out=$root/dist
build=1

while [[ $# -gt 0 ]]; do
    case $1 in
        --arch) arch=$2; shift 2 ;;
        --formats) formats=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        --skip-build) build=0; shift ;;
        -h | --help) sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

case $arch in
    x86_64) triple=x86_64-unknown-linux-gnu; nfpm_arch=amd64 ;;
    aarch64) triple=aarch64-unknown-linux-gnu; nfpm_arch=arm64 ;;
    *) echo "--arch must be x86_64 or aarch64" >&2; exit 2 ;;
esac

mkdir -p "$out"
out=$(cd "$out" && pwd)

version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
[[ -n $version ]] || { echo "no version in Cargo.toml" >&2; exit 1; }

binary=$root/target/$triple/release/airframe
if ((build)); then
    cargo build --manifest-path "$root/Cargo.toml" --release --target "$triple" -p airframe
fi
[[ -f $binary ]] || { echo "no binary at $binary" >&2; exit 1; }

# FHS-style tree, shared by every format below.
packroot=$out/airframe-$version-linux-$arch
stage=$packroot/airframe-$version
rm -rf "$packroot"
mkdir -p "$stage/bin" "$stage/share/applications" "$stage/share/icons/hicolor/256x256/apps" \
    "$stage/share/icons/hicolor/512x512/apps"
cp "$binary" "$stage/bin/airframe"
chmod +x "$stage/bin/airframe"
cp "$root/packaging/linux/airframe.desktop" "$stage/share/applications/airframe.desktop"

# Rasterise the icons from the embedded logo SVG.
cargo run --manifest-path "$root/Cargo.toml" --release -p airframe \
    --example export_app_icons -- "$out/.icons"
for size in 256x256 512x512; do
    cp "$out/.icons/airframe-${size%x*}.png" \
        "$stage/share/icons/hicolor/$size/apps/airframe.png"
done

want() { [[ " $formats " == *" $1 "* ]]; }

if want tar; then
    tar -C "$packroot" -czf "$out/airframe-$version-linux-$arch.tar.gz" "airframe-$version"
    echo "tar: $out/airframe-$version-linux-$arch.tar.gz"
fi

if want deb || want rpm; then
    if ! command -v nfpm >/dev/null; then
        case $arch in
            x86_64) nfpm_machine=x86_64 ;;
            aarch64) nfpm_machine=arm64 ;;
        esac
        mkdir -p "$out/.tools"
        curl -fsSL -o "$out/.tools/nfpm.tar.gz" \
            "https://github.com/goreleaser/nfpm/releases/download/v$NFPM_VERSION/nfpm_${NFPM_VERSION}_Linux_${nfpm_machine}.tar.gz"
        tar -C "$out/.tools" -xzf "$out/.tools/nfpm.tar.gz" nfpm
        PATH="$out/.tools:$PATH"
    fi
    sed -e "s|@VERSION@|$version|g" \
        -e "s|@ARCH@|$nfpm_arch|g" \
        -e "s|@PACKROOT@|$stage|g" \
        "$root/packaging/linux/nfpm.yaml.in" > "$out/.tools/nfpm.yaml"
    if want deb; then
        nfpm package -f "$out/.tools/nfpm.yaml" -p deb \
            -t "$out/airframe-$version-linux-$arch.deb"
        echo "deb: $out/airframe-$version-linux-$arch.deb"
    fi
    if want rpm; then
        nfpm package -f "$out/.tools/nfpm.yaml" -p rpm \
            -t "$out/airframe-$version-linux-$arch.rpm"
        echo "rpm: $out/airframe-$version-linux-$arch.rpm"
    fi
fi

if want appimage; then
    appdir=$packroot/AppDir
    mkdir -p "$appdir/usr"
    cp -r "$stage/bin" "$stage/share" "$appdir/usr/"
    cp "$root/packaging/linux/AppRun" "$appdir/AppRun"
    chmod +x "$appdir/AppRun"
    # Inside the image the entry point is AppRun, not a system-wide binary.
    sed 's|^Exec=.*|Exec=AppRun|' "$root/packaging/linux/airframe.desktop" \
        > "$appdir/airframe.desktop"
    cp "$stage/share/icons/hicolor/256x256/apps/airframe.png" "$appdir/airframe.png"

    tool=$out/.tools/appimagetool-$arch.AppImage
    if [[ ! -f $tool ]]; then
        mkdir -p "$out/.tools"
        curl -fsSL -o "$tool" "$APPIMAGETOOL_URL/appimagetool-$arch.AppImage"
    fi
    chmod +x "$tool"
    # The env var lets appimagetool (and its bundled mksquashfs) run without FUSE.
    APPIMAGE_EXTRACT_AND_RUN=1 "$tool" "$appdir" \
        "$out/AirFrame-$version-linux-$arch.AppImage"
    echo "AppImage: $out/AirFrame-$version-linux-$arch.AppImage"
fi
