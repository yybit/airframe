//! The protocol-agnostic session contract shared by every backend and the UI.
//!
//! A backend owns a worker that talks to the remote host. It reports what
//! happens through [`SessionEvent`]s and accepts input through
//! [`SessionCommand`]s. The UI never touches a protocol library directly.

use crossbeam_channel::{Receiver, Sender};

/// Modifier/button bits for [`SessionCommand::Pointer`].
pub mod buttons {
    pub const LEFT: u8 = 1 << 0;
    pub const MIDDLE: u8 = 1 << 1;
    pub const RIGHT: u8 = 1 << 2;
}

/// A key transition, carrying both coordinate systems the protocols use.
///
/// VNC addresses keys by X11 keysym; RDP and SPICE address them by PC/XT
/// scancode. The UI produces both at once so no backend has to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    pub down: bool,
    /// X11 keysym.
    pub keysym: u32,
    /// PC/XT scancode set 1.
    pub scancode: u32,
    /// Whether the scancode carries the `E0` extension prefix.
    pub extended: bool,
}

/// A rectangle of client-area-averaged input from the UI to a backend.
#[derive(Debug, Clone)]
pub enum SessionCommand {
    /// Pointer motion and/or button change. `buttons` is a mask of [`buttons`].
    Pointer {
        x: u16,
        y: u16,
        buttons: u8,
        /// True when only the position changed.
        moved: bool,
    },
    Key(KeyInput),
    /// Wheel motion in notches; positive `dy` scrolls up.
    Scroll { x: u16, y: u16, dx: i16, dy: i16 },
    /// Push local clipboard text to the remote session.
    ClipboardText(String),
    /// Ask the remote desktop to change resolution (RDP).
    Resize { width: u16, height: u16 },
    /// Tear the session down.
    Close,
}

/// A rectangular update to the client framebuffer.
///
/// `rgba` is tightly packed, top-down, `w * h * 4` bytes.
#[derive(Debug, Clone)]
pub struct FramePatch {
    /// Size of the whole framebuffer this patch belongs to.
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl FramePatch {
    /// A patch covering the entire framebuffer.
    pub fn full(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self { width, height, x: 0, y: 0, w: width, h: height, rgba }
    }
}

/// A remote cursor bitmap the UI draws over the framebuffer.
///
/// RDP and SPICE keep the cursor out of the framebuffer, so a client that
/// ignores this renders a desktop with no pointer.
#[derive(Debug, Clone)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    /// RGBA with premultiplied alpha, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Something a backend reports to the UI.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A human-readable connection stage, e.g. `"Authenticating…"`.
    Status(String),
    /// The session is up and its framebuffer has this size.
    Connected { name: String, width: u32, height: u32 },
    /// The remote desktop changed size; the client framebuffer must be reallocated.
    Resized { width: u32, height: u32 },
    /// A framebuffer update.
    Frame(FramePatch),
    /// The remote cursor bitmap changed.
    CursorShape(CursorImage),
    /// The remote cursor moved, appeared or disappeared.
    CursorPosition { x: i32, y: i32, visible: bool },
    /// The remote clipboard changed.
    ClipboardText(String),
    /// The remote session rang its bell.
    Bell,
    /// A recoverable-looking failure, reported before the session ends.
    Error(String),
    /// The server asked for a password the connection does not have. The UI
    /// should prompt for one; the session waits instead of retrying.
    NeedsPassword,
    /// The session ended.
    Disconnected,
}

/// The two ends of a running session.
pub struct SessionHandle {
    pub commands: Sender<SessionCommand>,
    pub events: Receiver<SessionEvent>,
}

/// Copy a patch into a full RGBA framebuffer, growing it when the remote
/// desktop has changed size.
///
/// Returns `true` when `framebuffer` was reallocated.
pub fn apply_patch(
    framebuffer: &mut Vec<u8>,
    size: &mut (u32, u32),
    patch: &FramePatch,
) -> bool {
    let mut resized = false;
    if *size != (patch.width, patch.height) {
        *size = (patch.width, patch.height);
        framebuffer.clear();
        framebuffer.resize((patch.width as usize) * (patch.height as usize) * 4, 0);
        resized = true;
    }
    if patch.w == 0 || patch.h == 0 {
        return resized;
    }

    let stride = patch.width as usize * 4;
    let row_bytes = patch.w as usize * 4;
    for row in 0..patch.h as usize {
        let src = row * row_bytes;
        let dst = (patch.y as usize + row) * stride + patch.x as usize * 4;
        if src + row_bytes > patch.rgba.len() || dst + row_bytes > framebuffer.len() {
            break;
        }
        framebuffer[dst..dst + row_bytes]
            .copy_from_slice(&patch.rgba[src..src + row_bytes]);
    }
    resized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_copies_into_the_right_rows() {
        let mut fb = vec![0u8; 4 * 3 * 4]; // 4x3
        let mut size = (4, 3);
        let patch = FramePatch {
            width: 4,
            height: 3,
            x: 1,
            y: 1,
            w: 2,
            h: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert!(!apply_patch(&mut fb, &mut size, &patch));
        let row = &fb[(4 + 1) * 4..(4 + 3) * 4];
        assert_eq!(row, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&fb[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn patch_reallocates_on_resize() {
        let mut fb = vec![];
        let mut size = (0, 0);
        let patch = FramePatch::full(2, 2, vec![9; 16]);
        assert!(apply_patch(&mut fb, &mut size, &patch));
        assert_eq!(size, (2, 2));
        assert_eq!(fb.len(), 16);
        assert_eq!(fb[0], 9);
    }
}
