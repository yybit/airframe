<div align="center">

<img src="assets/airframe-logo.svg" alt="AirFrame" width="120" />

# AirFrame

**一个原生远程桌面客户端，支持 VNC、RDP 和 SPICE。**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)]()
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](#从源码构建)

[简体中文](#) · [English](README.md)

<img src="apps/airframe/tests/snapshots/sidebar-chinese.png" alt="AirFrame 主界面（中文）" width="720" />

</div>

---

## 简介

AirFrame 是一个用 Rust 编写的原生远程桌面客户端。它把三种主流协议——VNC、RDP 和 SPICE——装进同一个轻量、GPU 加速的界面里，用统一的方式管理连接、输入和剪贴板，无需为每台机器各装一个客户端。

macOS、Windows 和 Linux 的预构建安装包发布在 [GitHub Releases](https://github.com/yybit/airframe/releases/latest)。

## 特性

- **三种协议，一个客户端** — VNC (RFB)、RDP、SPICE；新建连接时选协议，端口自动填好默认值
- **原生 GPU 界面** — 基于 egui/eframe (wgpu)，Linux (X11/Wayland)、macOS 和 Windows 上开箱即用
- **连接管理** — 分组、搜索、标签页（Ctrl+T 新建 / Ctrl+W 关闭 / Ctrl+Tab 切换）、`vnc://` / `rdp://` / `spice://` 链接导入分享
- **舒适的远程会话** — 悬浮工具栏、全屏 (F11)、三种缩放（适应窗口 / 1:1 / 拉伸）、只读模式
- **剪贴板与文本输入** — 与远程主机同步剪贴板文本；Ctrl+Shift+V 打开发送文本窗口，绕过键盘映射直接输入
- **键盘布局无关** — 按物理键位置转换按键（VNC 用 X11 keysym，RDP/SPICE 用 PC/XT scancode），本机的 Dvorak、AZERTY 等布局不会泄漏进远程会话
- **明暗主题 + 中英双语** — 界面内置中文与英文，主题支持深色 / 浅色
- **密码默认不落盘** — 密码只保存在内存中，除非显式勾选“记住密码”
- **纯粹 Rust** — 全工作区 `#![forbid(unsafe_code)]`，UI 快照测试覆盖主要界面

## 支持的协议

| 协议 | 默认端口 | 链接协议 | 后端 |
|------|---------|---------|------|
| VNC (RFB) | 5900 | `vnc://` | [`vnc`](https://crates.io/crates/vnc)（Raw / CopyRect / Hextile / ZRLE 解码） |
| RDP | 3389 | `rdp://` | [IronRDP](https://crates.io/crates/ironrdp)（自动安全协商、压缩、分辨率跟随视口） |
| SPICE | 5930 | `spice://` | [`spice-client`](https://crates.io/crates/spice-client) |

## 从源码构建

### 环境要求

- Rust **1.95** 或更新版本（`rustup update stable`）
- Linux 需要基本的窗口/输入系统库（Ubuntu/Debian 示例）：

  ```sh
  sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
  ```

  另外需要支持 Vulkan 的显卡驱动。
- macOS 使用 Xcode 命令行工具即可；Windows 安装面向 MSVC 的 Rust 工具链即可。

### 构建与运行

```sh
git clone https://github.com/yybit/airframe.git
cd airframe
cargo run --release      # 编译并启动
cargo test               # 运行测试（含 UI 快照测试）
```

### 发布打包

推送 `v*` 标签（或在 Actions 页手动触发工作流）即可构建并发布安装包到
[GitHub Releases](https://github.com/yybit/airframe/releases/latest)：

- **macOS** — 通用 `.dmg` / `.zip`（Apple Silicon + Intel）
- **Windows** — x64 `.msi` 安装包 / `.zip`
- **Linux** — x86_64 与 aarch64 的 `.deb` / `.rpm` / `.AppImage` / `.tar.gz`

CI 会在每次推送时于 Linux 和 Windows 上运行 clippy 与测试。
打包脚本也可以本地直接用：`packaging/macos/package.sh`、
`packaging/linux/package.sh --arch x86_64`，Windows 安装包用
`packaging/windows/airframe.wxs`（WiX）构建。

## 使用入门

1. 启动后首次使用会有引导界面，点击 **New connection** 新建连接
2. 选择协议（VNC / RDP / SPICE），填写主机地址与端口（默认端口已自动填好）、用户名和密码
3. 保存后从侧边栏双击即可连接；会话中可用悬浮工具栏切换全屏与缩放

**快捷键**

| 快捷键 | 作用 |
|--------|------|
| `Ctrl+T` | 新建连接 |
| `Ctrl+W` | 关闭当前会话 |
| `Ctrl+Tab` | 在会话之间切换 |
| `Ctrl+Shift+V` | 打开发送文本窗口 |
| `F11` | 全屏 |

## 数据存放位置

保存的连接与偏好设置以 JSON 明文存储（密码仅在勾选“记住密码”时写入）：

- Linux / Unix：`~/.config/airframe/airframe.json`（遵循 `$XDG_CONFIG_HOME`）
- macOS：`~/Library/Application Support/AirFrame/airframe.json`

## 项目结构

```
apps/airframe        桌面应用：界面、会话管理、输入与快捷键
crates/core          协议无关核心：连接模型、会话契约、本地存储
crates/vnc           VNC (RFB) 后端
crates/rdp           RDP 后端（基于 IronRDP，Tokio 运行时）
crates/spice         SPICE 后端
crates/ui            主题、图标、通用控件
packaging/macos      macOS 打包脚本（.app / 通用二进制）
packaging/linux      Linux 打包脚本（.deb / .rpm / .AppImage / .tar.gz）
packaging/windows    Windows 安装包定义（WiX .wxs）
```

架构上，UI 与各协议后端互不依赖，只通过 `airframe-core` 的会话契约（`SessionEvent` / `SessionCommand` 通道）通信——这也是三种后端可以互相替换、新增协议成本很低的原因。

## 项目状态

v0.1.0，仍处于早期开发阶段，接口与配置格式都可能变化。

## 参与贡献

欢迎 Issue 和 Pull Request。提交前请确保 `cargo test` 通过、`cargo clippy` 无警告。

## 许可证

本项目采用 **Apache-2.0** 许可证（见 [LICENSE](LICENSE)）。

内置的 [Noto Sans SC](assets/fonts/OFL.txt) 字体遵循 SIL Open Font License。
