//! The runtime half of a tab: one running session, its framebuffer, its
//! texture, and the input it has collected.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use airframe_core::{
    Connection, CursorImage, KeyInput, Lang, Protocol, SessionCommand, SessionEvent, SessionHandle,
    apply_patch, buttons,
};
use airframe_ui::State;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};

use crate::keymap;

/// The gap between the keys of a typed run.
///
/// Injecting text as keystrokes — the only way to paste into a console that has
/// no clipboard — overruns the guest's input queue if it is sent flat out, the
/// way key-by-key tools have always had to pace it.
const TYPE_INTERVAL: Duration = Duration::from_millis(4);

/// How the remote framebuffer is fitted into the viewport.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    /// Scale to fit, preserving the aspect ratio.
    Fit,
    /// Native pixels, centred and clipped.
    Actual,
    /// Fill the viewport, distorting if needed.
    Stretch,
}

impl Zoom {
    pub const ALL: [Zoom; 3] = [Zoom::Fit, Zoom::Actual, Zoom::Stretch];
}

/// A running session attached to a tab.
pub struct Session {
    pub connection: Connection,
    pub handle: SessionHandle,
    pub state: State,
    pub status: String,
    /// The interface language, so status lines follow the language picker.
    language: Lang,
    pub error: Option<String>,
    pub name: String,
    pub size: (u32, u32),
    pub zoom: Zoom,
    pub view_only: bool,
    /// Clipboard text the remote session sent us, waiting to be handed to the
    /// window's clipboard.
    pending_copy: Option<String>,
    /// Key events of text being typed into the session, still to go out, and
    /// when the next one may.
    typing: VecDeque<KeyInput>,
    next_key: Instant,
    /// Decays to zero; drives the bell flash.
    pub bell: f32,
    pub fps: f32,
    pub received_bytes: u64,

    texture: Option<TextureHandle>,
    texture_name: String,
    framebuffer: Vec<u8>,
    dirty: Option<(u32, u32, u32, u32)>,
    needs_full_upload: bool,

    /// When the next reconnect attempt is due, how many have failed in a row —
    /// reset once a session comes up — and whether this one is being closed on
    /// purpose.
    retry_due: Option<Instant>,
    retry_attempts: u32,
    closing: bool,

    cursor: Option<CursorImage>,
    cursor_texture: Option<TextureHandle>,
    cursor_texture_name: String,
    cursor_dirty: bool,
    pub cursor_pos: (i32, i32),
    pub cursor_visible: bool,

    frames_this_second: u32,
    fps_clock: Instant,

    /// Pointer state the viewport keeps for us.
    pub pointer: Option<(u16, u16)>,
    pub down_buttons: u8,
    /// Which modifiers were held at the end of the previous frame.
    pub modifiers: [bool; 4],
    /// Wheel movement too small to make a whole notch yet.
    pub scroll_accum: (f32, f32),
}

impl Session {
    pub fn new(connection: Connection, lang: Lang) -> Self {
        let handle = start(&connection);
        let name = connection.display_name().to_string();
        let view_only = connection.view_only;
        let texture_name = format!("airframe-session-{}", connection.id);
        let cursor_texture_name = format!("airframe-cursor-{}", connection.id);
        Self {
            connection,
            handle,
            state: State::Connecting,
            status: lang.pick("Starting…", "正在连接…").into(),
            language: lang,
            error: None,
            name,
            size: (0, 0),
            zoom: Zoom::Fit,
            view_only,
            pending_copy: None,
            typing: VecDeque::new(),
            next_key: Instant::now(),
            bell: 0.0,
            fps: 0.0,
            received_bytes: 0,
            texture: None,
            texture_name,
            framebuffer: Vec::new(),
            dirty: None,
            needs_full_upload: false,
            retry_due: None,
            retry_attempts: 0,
            closing: false,
            cursor: None,
            cursor_texture: None,
            cursor_texture_name,
            cursor_dirty: false,
            cursor_pos: (0, 0),
            cursor_visible: false,
            frames_this_second: 0,
            fps_clock: Instant::now(),
            pointer: None,
            down_buttons: 0,
            modifiers: [false; 4],
            scroll_accum: (0.0, 0.0),
        }
    }

    /// Drop the current session and start a fresh one for the same connection.
    pub fn restart(&mut self) {
        self.closing = false;
        self.retry_due = None;
        self.retry_attempts = 0;
        self.bring_up();
    }

    /// Replace the backend and forget everything that belonged to the old one.
    ///
    /// The retry bookkeeping is left alone: this is also how an automatic
    /// reconnect starts, and its backoff depends on how many attempts failed.
    fn bring_up(&mut self) {
        self.handle = start(&self.connection);
        self.state = State::Connecting;
        self.status = self.language.pick("Starting…", "正在连接…").into();
        self.error = None;
        self.texture = None;
        self.size = (0, 0);
        self.framebuffer.clear();
        self.dirty = None;
        self.needs_full_upload = true;
        self.cursor = None;
        self.cursor_texture = None;
        self.cursor_dirty = false;
        self.cursor_visible = false;
        self.received_bytes = 0;
        self.fps = 0.0;
        self.frames_this_second = 0;
        self.fps_clock = Instant::now();
        self.pointer = None;
        self.down_buttons = 0;
        self.modifiers = [false; 4];
        self.scroll_accum = (0.0, 0.0);
    }

    /// Tear the session down but keep the tab, so it can be reconnected.
    ///
    /// Closing on purpose is also what stops the automatic reconnect: the drop
    /// that follows is not something to retry.
    pub fn disconnect(&mut self) {
        self.closing = true;
        self.retry_due = None;
        self.retry_attempts = 0;
        self.send(SessionCommand::Close);
    }

    /// Queue another attempt after the server went away.
    ///
    /// A dropped connection is the thing a remote desktop is most likely to do,
    /// and the user's only remedy would be to click Reconnect and wait. So it
    /// retries by itself, backing off so a server that is down is not hammered,
    /// until the session is closed.
    fn schedule_retry(&mut self) {
        self.retry_attempts += 1;
        let wait = backoff(self.retry_attempts);
        self.retry_due = Some(Instant::now() + wait);
        self.state = State::Connecting;
        self.status = self.retry_status(wait);
    }

    /// Start the next attempt once it is due, and count down until then.
    fn drive_retry(&mut self, ctx: &Context) {
        let Some(due) = self.retry_due else {
            return;
        };
        let wait = due.saturating_duration_since(Instant::now());
        if wait.is_zero() {
            self.retry_due = None;
            self.bring_up();
            return;
        }
        // A session with no backend repaints for nothing else, so the countdown
        // has to ask for its own frames.
        ctx.request_repaint_after(wait);
        self.status = self.retry_status(wait);
    }

    /// What the status line reads while waiting: the last error if the backend
    /// gave one, so the reason for the retries is not hidden by them.
    fn retry_status(&self, wait: Duration) -> String {
        let seconds = wait.as_secs_f32().ceil().max(1.0) as u64;
        match self.language {
            Lang::En => match &self.error {
                Some(error) => format!("{error} · retrying in {seconds}s"),
                None => format!("Reconnecting in {seconds}s…"),
            },
            Lang::Zh => match &self.error {
                Some(error) => format!("{error} · {seconds} 秒后重试"),
                None => format!("{seconds} 秒后恢复连接…"),
            },
        }
    }

    pub fn send(&self, command: SessionCommand) {
        let _ = self.handle.commands.send(command);
    }

    /// The session's current picture, encoded as a PNG.
    ///
    /// `None` until the first frame has arrived.
    pub fn screenshot(&self) -> Option<Vec<u8>> {
        let (width, height) = self.size;
        let bytes = width as usize * height as usize * 4;
        if width == 0 || height == 0 || self.framebuffer.len() < bytes {
            return None;
        }
        encode_png(width, height, &self.framebuffer[..bytes])
    }

    pub fn texture(&self) -> Option<&TextureHandle> {
        self.texture.as_ref()
    }

    pub fn cursor_texture(&self) -> Option<&TextureHandle> {
        self.cursor_texture.as_ref()
    }

    pub fn cursor_image(&self) -> Option<&CursorImage> {
        self.cursor.as_ref()
    }

    /// Drain everything the backend produced since the last frame.
    ///
    /// Returns any text the remote session put on its clipboard, for the caller
    /// to hand to the window and to say so.
    pub fn pump(&mut self, ctx: &Context, lang: Lang) -> Option<String> {
        self.language = lang;
        while let Ok(event) = self.handle.events.try_recv() {
            self.apply(event);
        }

        let copied = self.pending_copy.take();

        // Typed text goes out a key at a time, spaced out: a remote input path
        // drops keys that arrive faster than the guest reads them. The deadline
        // keeps the pace independent of how fast frames happen to be running.
        let now = Instant::now();
        if self.next_key <= now
            && let Some(key) = self.typing.pop_front()
        {
            self.send(SessionCommand::Key(key));
            self.next_key = now + TYPE_INTERVAL;
        }
        if !self.typing.is_empty() {
            ctx.request_repaint_after(TYPE_INTERVAL);
        }

        self.drive_retry(ctx);

        self.upload(ctx);
        if self.cursor_dirty {
            self.cursor_dirty = false;
            self.upload_cursor(ctx);
        }

        self.frames_this_second += 1;
        let elapsed = self.fps_clock.elapsed();
        if elapsed >= Duration::from_millis(500) {
            let seconds = elapsed.as_secs_f32();
            self.fps = self.frames_this_second as f32 / seconds;
            self.frames_this_second = 0;
            self.fps_clock = Instant::now();
        }
        self.bell = (self.bell - ctx.input(|i| i.stable_dt)).max(0.0);
        copied
    }

    /// Type `text` into the remote session, key by key.
    ///
    /// A remote desktop has no way to receive pasted text: the protocols carry
    /// key codes. This is what makes pasting into a console — a QEMU console, a
    /// bare Linux tty — possible, because those have no clipboard of their own
    /// to paste from.
    ///
    /// The keys are queued and paced out by [`Session::pump`] rather than sent
    /// at once: a remote input path drops keys that arrive faster than the guest
    /// reads them, and a pasted line is exactly that burst.
    ///
    /// Returns how many characters could not be typed, which is everything
    /// outside what a US keyboard can produce.
    pub fn type_text(&mut self, text: &str) -> usize {
        let mut skipped = 0;
        let shift_held = self.modifiers[0];
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            // Carriage returns are dropped when they open a CRLF pair, so a
            // pasted line ending presses Enter once.
            if c == '\r' && chars.peek() == Some(&'\n') {
                continue;
            }
            let Some((key, shifted)) = keymap::character(c) else {
                skipped += 1;
                continue;
            };
            // Shift is only sent when the user is not already holding it, so
            // typing never leaves the remote keyboard in a stuck state.
            let press_shift = shifted && !shift_held;
            if press_shift {
                self.typing.push_back(keymap::SHIFT.input(true));
            }
            self.typing.push_back(key.input(true));
            self.typing.push_back(key.input(false));
            if press_shift {
                self.typing.push_back(keymap::SHIFT.input(false));
            }
        }
        skipped
    }

    fn apply(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::Status(status) => self.status = status,
            SessionEvent::Connected { name, width, height } => {
                self.name = name;
                self.state = State::Live;
                self.error = None;
                // Whatever the retries were for, this session is up.
                self.retry_due = None;
                self.retry_attempts = 0;
                self.resize(width, height);
            }
            SessionEvent::Resized { width, height } => self.resize(width, height),
            SessionEvent::Frame(patch) => {
                self.received_bytes += patch.rgba.len() as u64;
                if apply_patch(&mut self.framebuffer, &mut self.size, &patch) {
                    self.needs_full_upload = true;
                }
                let rect = (patch.x, patch.y, patch.w, patch.h);
                self.dirty = Some(match self.dirty {
                    Some(current) => union(current, rect),
                    None => rect,
                });
            }
            SessionEvent::CursorShape(image) => {
                self.cursor = Some(image);
                self.cursor_dirty = true;
            }
            SessionEvent::CursorPosition { x, y, visible } => {
                self.cursor_pos = (x, y);
                self.cursor_visible = visible;
            }
            SessionEvent::ClipboardText(text) => self.pending_copy = Some(text),
            SessionEvent::Bell => self.bell = 1.0,
            SessionEvent::Error(message) => {
                self.error = Some(message);
                self.state = State::Failed;
            }
            SessionEvent::Disconnected => {
                // A run of typed text belongs to the session it was made in: it
                // must not land in whatever the next connection turns out to be.
                self.typing.clear();
                if self.closing {
                    self.state = State::Idle;
                    self.status = self.language.pick("Disconnected", "已断开").into();
                } else {
                    // Any teardown the user did not ask for is worth another
                    // attempt; the backends report a dead worker as an error
                    // followed by this.
                    self.schedule_retry();
                }
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if self.size == (width, height) {
            return;
        }
        self.size = (width, height);
        self.framebuffer.clear();
        self.framebuffer.resize(width as usize * height as usize * 4, 0);
        self.texture = None;
        self.needs_full_upload = true;
        self.dirty = Some((0, 0, width, height));
    }

    /// Push this frame's changes into the session texture.
    fn upload(&mut self, ctx: &Context) {
        let (width, height) = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let Some(dirty) = self.dirty.take() else {
            return;
        };

        if self.needs_full_upload || self.texture.is_none() {
            self.needs_full_upload = false;
            let image = region_image(&self.framebuffer, width, 0, 0, width, height);
            match &mut self.texture {
                Some(texture) => texture.set(image, TextureOptions::NEAREST),
                None => {
                    self.texture =
                        Some(ctx.load_texture(&self.texture_name, image, TextureOptions::NEAREST));
                }
            }
        } else {
            let (x, y, w, h) = clamp_rect(dirty, width, height);
            if w == 0 || h == 0 {
                return;
            }
            let image = region_image(&self.framebuffer, width, x, y, w, h);
            if let Some(texture) = &mut self.texture {
                texture.set_partial([x as usize, y as usize], image, TextureOptions::NEAREST);
            }
        }
    }

    fn upload_cursor(&mut self, ctx: &Context) {
        let Some(cursor) = &self.cursor else {
            return;
        };
        if cursor.width == 0 || cursor.height == 0 {
            return;
        }
        let expected = cursor.width as usize * cursor.height as usize * 4;
        if cursor.rgba.len() < expected {
            return;
        }
        let mut pixels = Vec::with_capacity(cursor.width as usize * cursor.height as usize);
        for px in cursor.rgba[..expected].as_chunks::<4>().0 {
            pixels.push(Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3]));
        }
        let image = ColorImage::new([cursor.width as usize, cursor.height as usize], pixels);
        match &mut self.cursor_texture {
            Some(texture) => texture.set(image, TextureOptions::NEAREST),
            None => {
                self.cursor_texture = Some(ctx.load_texture(
                    &self.cursor_texture_name,
                    image,
                    TextureOptions::NEAREST,
                ));
            }
        }
    }

    /// Ask the remote desktop to change resolution, when it can.
    pub fn request_resize(&self, width: u16, height: u16) {
        if self.connection.protocol == Protocol::Rdp && !self.view_only {
            self.send(SessionCommand::Resize { width, height });
        }
    }

    /// The buttons a viewport click produced, as AirFrame's mask.
    pub fn buttons_from_egui(pointer: &egui::PointerState) -> u8 {
        let mut mask = 0;
        if pointer.button_down(egui::PointerButton::Primary) {
            mask |= buttons::LEFT;
        }
        if pointer.button_down(egui::PointerButton::Middle) {
            mask |= buttons::MIDDLE;
        }
        if pointer.button_down(egui::PointerButton::Secondary) {
            mask |= buttons::RIGHT;
        }
        mask
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.handle.commands.send(SessionCommand::Close);
    }
}

fn start(connection: &Connection) -> SessionHandle {
    match connection.protocol {
        Protocol::Vnc => airframe_vnc::start(connection),
        Protocol::Rdp => airframe_rdp::start(connection),
        Protocol::Spice => airframe_spice::start(connection),
    }
}

fn union(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    let x = a.0.min(b.0);
    let y = a.1.min(b.1);
    let right = (a.0 + a.2).max(b.0 + b.2);
    let bottom = (a.1 + a.3).max(b.1 + b.3);
    (x, y, right - x, bottom - y)
}

fn clamp_rect(rect: (u32, u32, u32, u32), width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x = rect.0.min(width);
    let y = rect.1.min(height);
    let w = rect.2.min(width - x);
    let h = rect.3.min(height - y);
    (x, y, w, h)
}

fn region_image(framebuffer: &[u8], fb_width: u32, x: u32, y: u32, w: u32, h: u32) -> ColorImage {
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    let stride = fb_width as usize * 4;
    for row in 0..h {
        let start = (y + row) as usize * stride + x as usize * 4;
        let end = start + w as usize * 4;
        if end > framebuffer.len() {
            break;
        }
        for px in framebuffer[start..end].as_chunks::<4>().0 {
            pixels.push(Color32::from_rgba_unmultiplied(px[0], px[1], px[2], px[3]));
        }
    }
    ColorImage::new([w as usize, h as usize], pixels)
}

/// How long to wait before retry `attempts` (1-based): 1, 2, 4, 8, 15, then a
/// steady 30 seconds.
fn backoff(attempts: u32) -> Duration {
    match attempts {
        0 | 1 => Duration::from_secs(1),
        2 => Duration::from_secs(2),
        3 => Duration::from_secs(4),
        4 => Duration::from_secs(8),
        5 => Duration::from_secs(15),
        _ => Duration::from_secs(30),
    }
}

/// Encode straight (unmultiplied) RGBA rows as a PNG.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A connection to a port that accepts the socket and then says nothing, so
    /// the backend stays quiet and the test can drive the events itself.
    fn quiet_session() -> (TcpListener, Session) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        let port = listener.local_addr().expect("the bound address").port();
        let mut connection = Connection::new("Quiet", Protocol::Vnc, "127.0.0.1");
        connection.port = port;
        (listener, Session::new(connection, Lang::En))
    }

    #[test]
    fn the_backoff_grows_and_then_holds() {
        assert_eq!(backoff(1), Duration::from_secs(1));
        assert_eq!(backoff(2), Duration::from_secs(2));
        assert_eq!(backoff(3), Duration::from_secs(4));
        assert_eq!(backoff(4), Duration::from_secs(8));
        assert_eq!(backoff(5), Duration::from_secs(15));
        assert_eq!(backoff(60), Duration::from_secs(30));
    }

    #[test]
    fn typed_text_is_paced_out_key_by_key() {
        let (_quiet, mut session) = quiet_session();
        assert_eq!(session.type_text("Hi"), 0, "both are on a US keyboard");
        assert_eq!(
            session.typing.len(),
            6,
            "shift, H down and up, then i down and up"
        );

        session.pump(&Context::default(), Lang::En);
        assert_eq!(session.typing.len(), 5, "one key a frame, not the whole run");

        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(5));
            session.pump(&Context::default(), Lang::En);
        }
        assert!(session.typing.is_empty(), "and the run does finish");

        assert_eq!(session.type_text("中"), 1, "a CJK character cannot be typed");
        assert!(session.typing.is_empty(), "so nothing is queued for it");
    }

    #[test]
    fn a_drop_takes_the_untyped_text_with_it() {
        let (_quiet, mut session) = quiet_session();
        session.state = State::Live;
        session.type_text("a line long enough to still be queued");

        session.apply(SessionEvent::Disconnected);
        assert!(session.typing.is_empty(), "it must not land in the next session");
    }

    #[test]
    fn a_dropped_session_retries_and_counts_up() {
        let (_quiet, mut session) = quiet_session();
        session.state = State::Live;

        session.apply(SessionEvent::Disconnected);
        assert!(session.retry_due.is_some(), "a drop is retried");
        assert_eq!(session.retry_attempts, 1, "the first retry is attempt one");
        assert!(matches!(session.state, State::Connecting));
        assert!(session.status.contains("Reconnect"), "status was {:?}", session.status);

        // Let each wait elapse, and let the attempt that follows fail again.
        for attempts in 2..=4 {
            session.retry_due = Some(Instant::now() - Duration::from_secs(1));
            let _ = session.pump(&Context::default(), Lang::En);
            session.apply(SessionEvent::Disconnected);
            assert_eq!(session.retry_attempts, attempts, "failures accumulate across attempts");
            assert!(matches!(session.state, State::Connecting));
        }
        assert_eq!(backoff(4), Duration::from_secs(8), "the wait grows with the failures");
    }

    #[test]
    fn closing_a_session_stops_the_retries() {
        let (_quiet, mut session) = quiet_session();
        session.state = State::Live;

        session.disconnect();
        session.apply(SessionEvent::Disconnected);
        assert!(session.retry_due.is_none(), "an explicit close is not retried");
        assert!(matches!(session.state, State::Idle));

        // And asking for a reconnect starts over, backoff included.
        session.schedule_retry();
        session.restart();
        assert!(session.retry_due.is_none(), "a fresh start forgets the failures");
        assert_eq!(session.retry_attempts, 0);
    }

    #[test]
    fn a_captured_frame_becomes_a_png() {
        let (_quiet, mut session) = quiet_session();
        assert!(session.screenshot().is_none(), "there is nothing to capture yet");

        session.size = (2, 1);
        session.framebuffer = vec![255, 0, 0, 255, 0, 128, 255, 255];
        let png = session.screenshot().expect("the frame is captured");
        assert_eq!(&png[1..4], b"PNG", "the bytes are a PNG");
    }

    #[test]
    fn clipboard_text_from_the_session_is_handed_to_the_window() {
        let (_quiet, mut session) = quiet_session();
        session.apply(SessionEvent::ClipboardText("from the remote".into()));
        assert_eq!(session.pending_copy.as_deref(), Some("from the remote"));
    }
}
