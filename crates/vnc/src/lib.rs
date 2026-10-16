//! VNC (RFB) backend, built on the `vnc` protocol crate.
//!
//! The `vnc` crate owns the RFB state machine and the framebuffer decoders
//! (Raw, CopyRect, Hextile, ZRLE); this module drives it from a worker thread,
//! maintains the client-side framebuffer, and translates between it and
//! AirFrame's protocol-agnostic session contract.

use std::cell::Cell;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use airframe_core::session::SessionCommand as Cmd;
use airframe_core::{Connection, FramePatch, SessionEvent, SessionHandle};
use crossbeam_channel::{Receiver, Sender, unbounded};
use vnc::client::{AuthChoice, AuthMethod, Client, Event};
use vnc::{Encoding, PixelFormat, Rect};

/// Pixel format we ask every server for: 32bpp little-endian true colour with
/// the channels in R, G, B, pad byte order. RFB hands us the wire bytes
/// verbatim, so the decoders' output is already RGBA and only needs its alpha
/// byte forced opaque.
fn rgba8888() -> PixelFormat {
    PixelFormat {
        bits_per_pixel: 32,
        depth: 24,
        big_endian: false,
        true_colour: true,
        red_max: 255,
        green_max: 255,
        blue_max: 255,
        red_shift: 0,
        green_shift: 8,
        blue_shift: 16,
    }
}

/// Start a VNC session. Errors surface as [`SessionEvent::Error`] followed by
/// [`SessionEvent::Disconnected`].
pub fn start(connection: &Connection) -> SessionHandle {
    let (cmd_tx, cmd_rx) = unbounded();
    let (evt_tx, evt_rx) = unbounded();

    let conn = connection.clone();
    let worker_tx = evt_tx.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("vnc-{}", conn.id))
        .spawn(move || {
            if let Err(message) = run(&conn, cmd_rx, &worker_tx) {
                let _ = worker_tx.send(SessionEvent::Error(message));
                let _ = worker_tx.send(SessionEvent::Disconnected);
            }
        });

    if let Err(err) = spawned {
        let _ = evt_tx.send(SessionEvent::Error(format!("could not start the VNC worker: {err}")));
        let _ = evt_tx.send(SessionEvent::Disconnected);
    }

    SessionHandle { commands: cmd_tx, events: evt_rx }
}

fn run(connection: &Connection, cmd_rx: Receiver<Cmd>, evt_tx: &Sender<SessionEvent>) -> Result<(), String> {
    let addr = format!("{}:{}", connection.host, connection.port);
    let sock = resolve(&addr)?;

    evt_tx.send(SessionEvent::Status(format!("Connecting to {addr}…"))).ok();

    let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(10))
        .map_err(|e| format!("could not reach {addr}: {e}"))?;
    stream.set_nodelay(true).ok();

    let password = connection.password.clone();
    let asked_for_password = Cell::new(false);
    let mut client = Client::from_tcp_stream(stream, connection.vnc.shared, |methods| {
        let offers_password =
            methods.iter().any(|m| matches!(m, AuthMethod::Password));
        let offers_none = methods.iter().any(|m| matches!(m, AuthMethod::None));
        if offers_password {
            return match &password {
                Some(pw) => {
                    let mut key = [0u8; 8];
                    let bytes = pw.as_bytes();
                    let n = bytes.len().min(8);
                    key[..n].copy_from_slice(&bytes[..n]);
                    Some(AuthChoice::Password(key))
                }
                None => {
                    asked_for_password.set(true);
                    None
                }
            };
        }
        if offers_none { Some(AuthChoice::None) } else { None }
    })
    .map_err(|e| {
        if asked_for_password.get() {
            "the server asked for a VNC password, but none is set for this connection".to_string()
        } else {
            format!("VNC handshake failed: {e}")
        }
    })?;

    let name = client.name().to_string();
    let (width, height) = client.size();
    evt_tx
        .send(SessionEvent::Connected { name, width: width as u32, height: height as u32 })
        .ok();

    client
        .set_format(rgba8888())
        .map_err(|e| format!("could not request the RGB8888 pixel format: {e}"))?;
    client
        .set_encodings(&[Encoding::Zrle, Encoding::Hextile, Encoding::CopyRect, Encoding::Raw])
        .map_err(|e| format!("could not negotiate encodings: {e}"))?;

    let mut fb = Framebuffer::new(width as u32, height as u32);
    let mut last_pointer = (0u16, 0u16);

    request_full(&mut client, width, height)?;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::Pointer { x, y, buttons, .. } => {
                    last_pointer = (x, y);
                    if let Err(e) = client.send_pointer_event(buttons, x, y) {
                        log::debug!("pointer event lost: {e}");
                        return Ok(());
                    }
                }
                Cmd::Key(key) => {
                    if let Err(e) = client.send_key_event(key.down, key.keysym) {
                        log::debug!("key event lost: {e}");
                        return Ok(());
                    }
                }
                Cmd::Scroll { x, y, dx, dy } => {
                    last_pointer = (x, y);
                    for _ in 0..dy.unsigned_abs() {
                        let button = if dy > 0 { 4 } else { 5 };
                        wheel(&mut client, button, x, y);
                    }
                    for _ in 0..dx.unsigned_abs() {
                        let button = if dx > 0 { 7 } else { 6 };
                        wheel(&mut client, button, x, y);
                    }
                }
                Cmd::ClipboardText(text) => {
                    let _ = client.update_clipboard(&text);
                }
                Cmd::Resize { .. } => {}
                Cmd::Close => return Ok(()),
            }
        }

        let mut idle = true;
        while let Some(event) = client.poll_event() {
            idle = false;
            match event {
                Event::PutPixels(rect, pixels) => {
                    if fb.blit(&rect, &pixels) {
                        let patch = fb.patch(&rect);
                        evt_tx.send(SessionEvent::Frame(patch)).ok();
                    }
                }
                Event::CopyPixels { src, dst } => {
                    if let Some(dst) = fb.copy(&src, &dst) {
                        let patch = fb.patch(&dst);
                        evt_tx.send(SessionEvent::Frame(patch)).ok();
                    }
                }
                Event::Resize(w, h) => {
                    fb = Framebuffer::new(w as u32, h as u32);
                    evt_tx.send(SessionEvent::Resized { width: w as u32, height: h as u32 }).ok();
                    request_full(&mut client, w, h)?;
                }
                Event::EndOfFrame => {
                    let (w, h) = client.size();
                    let full = Rect { left: 0, top: 0, width: w, height: h };
                    let _ = client.request_update(full, true);
                }
                Event::Clipboard(text) => {
                    evt_tx.send(SessionEvent::ClipboardText(text)).ok();
                }
                Event::Bell => {
                    evt_tx.send(SessionEvent::Bell).ok();
                }
                Event::SetColourMap { .. } => {
                    log::warn!("server sent a colour map despite true-colour negotiation");
                }
                Event::SetCursor { .. } => {}
                Event::Disconnected(reason) => {
                    if let Some(err) = reason {
                        evt_tx.send(SessionEvent::Error(format!("VNC session ended: {err}"))).ok();
                    }
                    evt_tx.send(SessionEvent::Disconnected).ok();
                    return Ok(());
                }
            }
        }

        let _ = last_pointer;
        if idle {
            std::thread::sleep(Duration::from_millis(3));
        }
    }
}

fn request_full(client: &mut Client, width: u16, height: u16) -> Result<(), String> {
    let full = Rect { left: 0, top: 0, width, height };
    client.request_update(full, false).map_err(|e| format!("could not request the framebuffer: {e}"))
}

fn wheel(client: &mut Client, button: u8, x: u16, y: u16) {
    let mask = 1u8 << (button - 1);
    let _ = client.send_pointer_event(mask, x, y);
    let _ = client.send_pointer_event(0, x, y);
}

fn resolve(addr: &str) -> Result<std::net::SocketAddr, String> {
    addr.to_socket_addrs()
        .map_err(|e| format!("could not resolve {addr}: {e}"))?
        .next()
        .ok_or_else(|| format!("{addr} resolved to no addresses"))
}

/// The client-side copy of the remote framebuffer, in RGBA8888.
struct Framebuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Framebuffer {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![0; width as usize * height as usize * 4] }
    }

    /// Copy a wire-format rectangle into the framebuffer, forcing alpha opaque.
    fn blit(&mut self, rect: &Rect, pixels: &[u8]) -> bool {
        let (rw, rh) = (rect.width as usize, rect.height as usize);
        if rw == 0 || rh == 0 || pixels.len() < rw * rh * 4 {
            return false;
        }
        if rect.left as u32 + rect.width as u32 > self.width
            || rect.top as u32 + rect.height as u32 > self.height
        {
            return false;
        }
        let stride = self.width as usize * 4;
        for row in 0..rh {
            let src = row * rw * 4;
            let dst = (rect.top as usize + row) * stride + rect.left as usize * 4;
            let src_row = &pixels[src..src + rw * 4];
            let dst_row = &mut self.data[dst..dst + rw * 4];
            dst_row.copy_from_slice(src_row);
            for px in dst_row.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
        }
        true
    }

    /// Server-side copy of a rectangle, clamped to the framebuffer.
    fn copy(&mut self, src: &Rect, dst: &Rect) -> Option<Rect> {
        let w = src.width.min(dst.width);
        let h = src.height.min(dst.height);
        if w == 0 || h == 0 {
            return None;
        }
        if src.left as u32 + w as u32 > self.width
            || src.top as u32 + h as u32 > self.height
            || dst.left as u32 + w as u32 > self.width
            || dst.top as u32 + h as u32 > self.height
        {
            return None;
        }
        let stride = self.width as usize * 4;
        let row_bytes = w as usize * 4;
        let mut scratch = vec![0u8; row_bytes * h as usize];
        for row in 0..h as usize {
            let s = (src.top as usize + row) * stride + src.left as usize * 4;
            scratch[row * row_bytes..(row + 1) * row_bytes]
                .copy_from_slice(&self.data[s..s + row_bytes]);
        }
        for row in 0..h as usize {
            let d = (dst.top as usize + row) * stride + dst.left as usize * 4;
            self.data[d..d + row_bytes]
                .copy_from_slice(&scratch[row * row_bytes..(row + 1) * row_bytes]);
        }
        Some(Rect { left: dst.left, top: dst.top, width: w, height: h })
    }

    fn patch(&self, rect: &Rect) -> FramePatch {
        let (w, h) = (rect.width as usize, rect.height as usize);
        let stride = self.width as usize * 4;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            let start = (rect.top as usize + row) * stride + rect.left as usize * 4;
            rgba.extend_from_slice(&self.data[start..start + w * 4]);
        }
        FramePatch {
            width: self.width,
            height: self.height,
            x: rect.left as u32,
            y: rect.top as u32,
            w: rect.width as u32,
            h: rect.height as u32,
            rgba,
        }
    }
}
