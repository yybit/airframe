# macOS packaging

    # on a Mac, with the Xcode command line tools installed
    packaging/macos/package.sh                    # for this Mac's architecture
    packaging/macos/package.sh --arch universal   # arm64 + x86_64

`package.sh` runs `cargo build --release`, assembles `AirFrame.app` (Info.plist
from `Info.plist.in`, icon baked from `crates/ui/src/app_icon.rs` by
`examples/export_app_icon.rs`), ad-hoc signs the bundle when `codesign` is
available and writes `dist/AirFrame-<version>-macos-<arch>.zip`.

## Cross-building from Linux

Also works, given a macOS SDK:

    SDKROOT=~/sdk/MacOSX15.5.sdk packaging/macos/cross-build.sh --target aarch64-apple-darwin
    packaging/macos/package.sh --arch aarch64 --skip-build \
        --binary target/aarch64-apple-darwin/release/airframe

`--target x86_64-apple-darwin` with `--arch x86_64` builds the Intel twin. Either
way `package.sh` needs `--arch` spelled out when cross-building: its default is
the *host* architecture.

The SDK is needed because zig's bundled Darwin libc is a bare `libSystem.tbd` —
no `libobjc`, no AppKit / CoreGraphics / Metal / QuartzCore — and a winit/wgpu
app links against all of those. The `.tbd`-only SDKs published at
<https://github.com/joseluisq/macosx-sdks> are enough (macOS 15.5 is 78 MB), but
note that Apple licenses its SDK for use on Apple hardware only, so this is a
convenience for local builds rather than something a public build should rely
on.

`cross-build.sh` drives two small wrappers, because three things need adapting:

* `zig-cc.sh`/`zig-ar.sh` reach zig through the `CC_<triple>`, `AR_<triple>` and
  `CARGO_TARGET_<TRIPLE>_LINKER` variables that `cc-rs` and rustc read.
* cc-rs spells the arch `arm64-apple-macosx`, which zig's target parser rejects,
  so the wrapper drops `--target=` and pins the target itself.
* zig's linker ignores `-isysroot` when resolving `-l`/`-framework`, so the
  wrapper passes the SDK as `-L`/`-F` as well.

## Opening the result

The main executable carries zig's ad-hoc signature (an arm64 executable without
one will not start on Apple Silicon), but the bundle is not signed with a
Developer ID and is not notarised, so a copy that travelled through a browser or
mail client is quarantined by Gatekeeper. Either right-click → Open, or:

    xattr -dr com.apple.quarantine AirFrame.app
