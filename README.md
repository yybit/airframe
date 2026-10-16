<div align="center">

<img src="assets/airframe-logo.svg" alt="AirFrame" width="120" />

# AirFrame

**One native remote desktop for VNC, RDP and SPICE.**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)]()
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](#building-from-source)

[English](#) · [简体中文](README.zh-CN.md)

<img src="apps/airframe/tests/snapshots/sidebar-dark.png" alt="AirFrame main window" width="720" />

</div>

---

## About

AirFrame is a native remote-desktop client written in Rust. It puts the three major protocols — VNC, RDP and SPICE — into one lightweight, GPU-accelerated interface, with unified handling of connections, input and clipboard, so you don't need a different client for every machine.

Prebuilt installers for macOS, Windows and Linux are published on [GitHub Releases](https://github.com/yybit/airframe/releases/latest).

## Features

- **Three protocols, one client** — VNC (RFB), RDP and SPICE; pick a protocol when creating a connection and the default port fills itself in
- **Native GPU-rendered UI** — built on egui/eframe (wgpu); works out of the box on Linux (X11/Wayland), macOS and Windows
- **Connection management** — groups, search, tabs (Ctrl+T new / Ctrl+W close / Ctrl+Tab switch), `vnc://` / `rdp://` / `spice://` links for sharing and importing
- **Comfortable sessions** — overlay toolbar, fullscreen (F11), three zoom modes (fit / 1:1 / stretch), view-only mode
- **Clipboard & text input** — clipboard text sync with the remote host; Ctrl+Shift+V opens a send-text window that bypasses key mapping entirely
- **Keyboard-layout independent** — keys are translated from their *physical* position (X11 keysyms for VNC, PC/XT scancodes for RDP/SPICE), so a Dvorak or AZERTY layout on your machine never leaks into the remote session
- **Dark & light themes + English / 中文** — the interface ships in English and Chinese
- **Passwords stay off disk by default** — passwords live in memory for the current run only, unless you explicitly tick "remember password"
- **Pure Rust** — `#![forbid(unsafe_code)]` across the workspace, with UI snapshot tests covering the main screens

## Supported protocols

| Protocol | Default port | URL scheme | Backend |
|----------|-------------|-----------|---------|
| VNC (RFB) | 5900 | `vnc://` | [`vnc`](https://crates.io/crates/vnc) (Raw / CopyRect / Hextile / ZRLE decoding) |
| RDP | 3389 | `rdp://` | [IronRDP](https://crates.io/crates/ironrdp) (automatic security negotiation, compression, resolution follows the viewport) |
| SPICE | 5930 | `spice://` | [`spice-client`](https://crates.io/crates/spice-client) |

## Building from source

### Requirements

- Rust **1.95** or newer (`rustup update stable`)
- On Linux, the basic windowing/input system libraries (Ubuntu/Debian example):

  ```sh
  sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
  ```

  A GPU with Vulkan support is also required.
- On macOS, the Xcode command line tools are enough. On Windows, a Rust
  toolchain targeting MSVC is enough.

### Build & run

```sh
git clone https://github.com/yybit/airframe.git
cd airframe
cargo run --release      # build and launch
cargo test               # run tests (includes UI snapshot tests)
```

### Release packaging

Pushing a `v*` tag (or running the workflow manually from the Actions tab)
builds and publishes installers to [GitHub Releases](https://github.com/yybit/airframe/releases/latest):

- **macOS** — universal `.dmg` / `.zip` (Apple Silicon + Intel)
- **Windows** — x64 `.msi` installer / `.zip`
- **Linux** — x86_64 & aarch64 `.deb` / `.rpm` / `.AppImage` / `.tar.gz`

CI also runs clippy and the test suite on Linux and Windows for every push.
The packaging scripts work locally too: `packaging/macos/package.sh`,
`packaging/linux/package.sh --arch x86_64`, and
`packaging/windows/airframe.wxs` (WiX) for the Windows installer.

## Getting started

1. First launch shows an onboarding screen — click **New connection** to create one
2. Pick a protocol (VNC / RDP / SPICE), fill in the host and port (default pre-filled), username and password
3. Save, double-click the entry in the sidebar to connect; use the overlay toolbar for fullscreen and zoom while in a session

**Keyboard shortcuts**

| Shortcut | Action |
|----------|--------|
| `Ctrl+T` | New connection |
| `Ctrl+W` | Close current session |
| `Ctrl+Tab` | Switch between sessions |
| `Ctrl+Shift+V` | Open the send-text window |
| `F11` | Fullscreen |

## Where data lives

Saved connections and preferences are stored as plain JSON (passwords are written only when "remember password" is ticked):

- Linux / Unix: `~/.config/airframe/airframe.json` (respects `$XDG_CONFIG_HOME`)
- macOS: `~/Library/Application Support/AirFrame/airframe.json`

## Project layout

```
apps/airframe        Desktop app: UI, session management, input, shortcuts
crates/core          Protocol-agnostic core: connection model, session contract, on-disk store
crates/vnc           VNC (RFB) backend
crates/rdp           RDP backend (IronRDP on a Tokio runtime)
crates/spice         SPICE backend
crates/ui            Theme, icons, shared widgets
packaging/macos      macOS packaging scripts (.app / universal binary)
packaging/linux      Linux packaging scripts (.deb / .rpm / .AppImage / .tar.gz)
packaging/windows    Windows installer definition (WiX .wxs)
```

Architecturally, the UI and the protocol backends don't depend on each other — they only communicate through the protocol-agnostic session contract in `airframe-core` (`SessionEvent` / `SessionCommand` channels). That is what keeps the three backends interchangeable and new protocols cheap to add.

## Status

v0.1.0, early days — interfaces and on-disk formats may still change.

## Contributing

Issues and pull requests are welcome. Please make sure `cargo test` passes and `cargo clippy` is warning-free before submitting.

## License

Licensed under **Apache-2.0** — see [LICENSE](LICENSE).

The bundled [Noto Sans SC](assets/fonts/OFL.txt) font is licensed under the SIL Open Font License.
