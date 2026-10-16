//! RDP backend, built on IronRDP's high-level client.
//!
//! IronRDP owns the connection sequence, the security/session negotiation and
//! the bitmap codecs. This module runs it on a dedicated Tokio runtime, feeds
//! it input from AirFrame's session contract, and republishes its frame and
//! pointer output as protocol-agnostic [`SessionEvent`]s.

use airframe_core::session::SessionCommand as Cmd;use airframe_core::{
    Connection, CursorImage, FramePatch, RdpSecurity, SessionEvent, SessionHandle, buttons,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use ironrdp::client::config::{Config, ConfigBuilder, Destination};
use ironrdp::client::rdp::{RdpClient, RdpInputEvent, RdpOutputEvent};
use ironrdp::pdu::input::MousePdu;
use ironrdp::pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp::pdu::input::mouse::PointerFlags;
use tokio::sync::mpsc::UnboundedSender;

/// Remote desktop size used when the connection does not pin one.
const DEFAULT_DESKTOP: (u16, u16) = (1440, 900);

/// Start an RDP session. Errors surface as [`SessionEvent::Error`] followed by
/// [`SessionEvent::Disconnected`].
pub fn start(connection: &Connection) -> SessionHandle {
    let (cmd_tx, cmd_rx) = unbounded();
    let (evt_tx, evt_rx) = unbounded();

    let conn = connection.clone();
    let worker_tx = evt_tx.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("rdp-{}", conn.id))
        .spawn(move || {
            if let Err(message) = run(&conn, cmd_rx, &worker_tx) {
                let _ = worker_tx.send(SessionEvent::Error(message));
                let _ = worker_tx.send(SessionEvent::Disconnected);
            }
        });

    if let Err(err) = spawned {
        let _ = evt_tx.send(SessionEvent::Error(format!("could not start the RDP worker: {err}")));
        let _ = evt_tx.send(SessionEvent::Disconnected);
    }

    SessionHandle { commands: cmd_tx, events: evt_rx }
}

fn run(
    connection: &Connection,
    cmd_rx: Receiver<Cmd>,
    evt_tx: &Sender<SessionEvent>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("could not start the RDP runtime: {e}"))?;

    runtime.block_on(async move {
        let config = build_config(connection)?;
        let (out_tx, mut out_rx) = tokio::sync::mpsc::channel(4);
        let client = RdpClient::new(config, out_tx);
        let input = client.input_sender();

        std::thread::Builder::new()
            .name(format!("rdp-input-{}", connection.id))
            .spawn(move || pump_commands(cmd_rx, input))
            .map_err(|e| format!("could not start the RDP input pump: {e}"))?;

        evt_tx.send(SessionEvent::Status("Negotiating RDP session…".into())).ok();

        // The client and the frame consumer share one task: `RdpClient::run`
        // is driven until it returns, at which point it drops the output
        // sender and the consumer's `recv` yields `None`.
        let connection_display_name = connection.display_name().to_string();
        let consumer = consume(&connection_display_name, &mut out_rx, evt_tx.clone());        tokio::join!(client.run(), consumer);

        evt_tx.send(SessionEvent::Disconnected).ok();
        Ok(())
    })
}

/// Republish IronRDP's output as AirFrame session events.
async fn consume(
    name: &str,
    out_rx: &mut tokio::sync::mpsc::Receiver<RdpOutputEvent>,
    evt_tx: Sender<SessionEvent>,
) {
    let mut announced = false;
    while let Some(event) = out_rx.recv().await {
        match event {
            RdpOutputEvent::Image { buffer, width, height } => {
                let (w, h) = (width.get() as u32, height.get() as u32);
                if !announced {
                    announced = true;
                    evt_tx
                        .send(SessionEvent::Connected {
                            name: name.to_string(),
                            width: w,
                            height: h,
                        })
                        .ok();
                }
                evt_tx.send(SessionEvent::Frame(FramePatch::full(w, h, to_rgba(&buffer)))).ok();
            }
            RdpOutputEvent::ConnectionFailure(err) => {
                evt_tx.send(SessionEvent::Error(describe(&err))).ok();
            }
            RdpOutputEvent::PointerHidden | RdpOutputEvent::PointerDefault => {
                evt_tx.send(SessionEvent::CursorPosition { x: 0, y: 0, visible: false }).ok();
            }
            RdpOutputEvent::PointerPosition { x, y } => {
                evt_tx
                    .send(SessionEvent::CursorPosition {
                        x: i32::from(x),
                        y: i32::from(y),
                        visible: true,
                    })
                    .ok();
            }
            RdpOutputEvent::PointerBitmap(pointer) => {
                let image = CursorImage {
                    width: u32::from(pointer.width),
                    height: u32::from(pointer.height),
                    hotspot_x: u32::from(pointer.hotspot_x),
                    hotspot_y: u32::from(pointer.hotspot_y),
                    rgba: pointer.bitmap_data.clone(),
                };
                evt_tx.send(SessionEvent::CursorShape(image)).ok();
            }
            RdpOutputEvent::Terminated(result) => {
                if let Err(err) = result {
                    evt_tx.send(SessionEvent::Error(format!("RDP session ended: {err}"))).ok();
                }
            }
        }
    }
}

fn build_config(connection: &Connection) -> Result<Config, String> {
    if connection.username.trim().is_empty() {
        return Err("RDP needs a username; add one to this connection".into());
    }
    let address = connection.address();
    let destination = Destination::new(address.clone())
        .map_err(|e| format!("{address} is not a valid RDP destination: {e}"))?;

    let (width, height) = match (connection.rdp.width, connection.rdp.height) {
        (w, h) if w > 0 && h > 0 => (w, h),
        _ => DEFAULT_DESKTOP,
    };

    let mut builder = ConfigBuilder::new()
        .with_destination(destination)
        .with_username(connection.username.clone())
        .with_password(connection.password.clone().unwrap_or_default())
        .with_domain(connection.domain.clone())
        .with_client_name("AirFrame")
        .with_client_dir("")
        .with_desktop_width(width)
        .with_desktop_height(height)
        .with_compression(connection.rdp.compression);

    builder = match connection.rdp.security {
        RdpSecurity::Automatic => builder.with_tls(true).with_credssp(true),
        RdpSecurity::Tls => builder.with_tls(true).with_credssp(false),
        RdpSecurity::Nla => builder.with_credssp(true),
    };

    builder.build().map_err(|e| format!("RDP configuration is incomplete: {e}"))
}

fn describe(err: &ironrdp::connector::ConnectorError) -> String {
    format!("RDP connection failed: {err}")
}

/// Read commands until the UI drops the session and post them to IronRDP.
fn pump_commands(cmd_rx: Receiver<Cmd>, input: UnboundedSender<RdpInputEvent>) {
    let mut down_buttons = 0u8;
    while let Ok(command) = cmd_rx.recv() {
        let sent = match command {
            Cmd::Pointer { x, y, buttons, moved } => {
                let events = pointer_events(&mut down_buttons, x, y, buttons, moved);
                send_fast_path(&input, events)
            }
            Cmd::Scroll { x, y, dx, dy } => send_fast_path(&input, wheel_events(x, y, dx, dy)),
            Cmd::Key(key) => send_fast_path(&input, vec![keyboard_event(&key)]),
            Cmd::Resize { width, height } => input
                .send(RdpInputEvent::Resize {
                    width,
                    height,
                    scale_factor: 100,
                    physical_size: None,
                })
                .is_ok(),
            Cmd::ClipboardText(_) => true,
            Cmd::Close => {
                let _ = input.send(RdpInputEvent::Close);
                return;
            }
        };
        if !sent {
            return;
        }
    }
}

fn send_fast_path(
    input: &UnboundedSender<RdpInputEvent>,
    events: Vec<FastPathInputEvent>,
) -> bool {
    events
        .into_iter()
        .all(|event| input.send(RdpInputEvent::FastPath(std::iter::once(event).collect())).is_ok())
}

fn pointer_events(down_buttons: &mut u8, x: u16, y: u16, buttons: u8, moved: bool) -> Vec<FastPathInputEvent> {
    let mut events = Vec::new();
    let changed = *down_buttons ^ buttons;

    if moved || changed == 0 {
        events.push(mouse(x, y, PointerFlags::MOVE, 0));
    }
    for bit in [buttons::LEFT, buttons::MIDDLE, buttons::RIGHT] {
        if changed & bit == 0 {
            continue;
        }
        let flag = match bit {
            buttons::LEFT => PointerFlags::LEFT_BUTTON,
            buttons::MIDDLE => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
            _ => PointerFlags::RIGHT_BUTTON,
        };
        let flag = if buttons & bit != 0 { flag | PointerFlags::DOWN } else { flag };
        events.push(mouse(x, y, flag, 0));
    }
    *down_buttons = buttons;
    events
}

fn wheel_events(x: u16, y: u16, dx: i16, dy: i16) -> Vec<FastPathInputEvent> {
    let mut events = Vec::new();
    if dy != 0 {
        events.push(mouse(x, y, PointerFlags::VERTICAL_WHEEL, dy));
    }
    if dx != 0 {
        events.push(mouse(x, y, PointerFlags::HORIZONTAL_WHEEL, dx));
    }
    events
}

fn mouse(x: u16, y: u16, flags: PointerFlags, wheel: i16) -> FastPathInputEvent {
    FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        number_of_wheel_rotation_units: wheel,
        x_position: x,
        y_position: y,
    })
}

fn keyboard_event(key: &airframe_core::KeyInput) -> FastPathInputEvent {
    let mut flags = KeyboardFlags::empty();
    if !key.down {
        flags |= KeyboardFlags::RELEASE;
    }
    if key.extended {
        flags |= KeyboardFlags::EXTENDED;
    }
    FastPathInputEvent::KeyboardEvent(flags, key.scancode as u8)
}

/// IronRDP packs pixels as `(r << 16) | (g << 8) | b`; unpack into RGBA.
fn to_rgba(buffer: &[u32]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(buffer.len() * 4);
    for pixel in buffer {
        rgba.push((pixel >> 16) as u8);
        rgba.push((pixel >> 8) as u8);
        rgba.push(*pixel as u8);
        rgba.push(255);
    }
    rgba
}
