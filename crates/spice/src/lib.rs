//! SPICE backend, built on the `spice-client` crate.
//!
//! `spice-client` keeps two parallel front ends: `SpiceClient` manages display
//! channels and `SpiceClientShared` manages input *and* exposes the display
//! surface, but neither publishes the server-assigned channel ids. AirFrame
//! therefore drives `SpiceClientShared` and discovers the two ids it needs by
//! probing once the connection is up.

use std::sync::Arc;
use std::time::Duration;

use airframe_core::session::SessionCommand as Cmd;
use airframe_core::{Connection, FramePatch, SessionEvent, SessionHandle, buttons};
use crossbeam_channel::{Receiver, Sender, unbounded};
use spice_client::SpiceClientShared;
use spice_client::channels::MouseButton;

/// How long to wait between framebuffer checks.
const POLL: Duration = Duration::from_millis(8);

/// Start a SPICE session. Errors surface as [`SessionEvent::Error`] followed by
/// [`SessionEvent::Disconnected`].
pub fn start(connection: &Connection) -> SessionHandle {
    let (cmd_tx, cmd_rx) = unbounded();
    let (evt_tx, evt_rx) = unbounded();

    let conn = connection.clone();
    let worker_tx = evt_tx.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("spice-{}", conn.id))
        .spawn(move || {
            if let Err(message) = run(&conn, cmd_rx, &worker_tx) {
                let _ = worker_tx.send(SessionEvent::Error(message));
                let _ = worker_tx.send(SessionEvent::Disconnected);
            }
        });

    if let Err(err) = spawned {
        let _ = evt_tx.send(SessionEvent::Error(format!("could not start the SPICE worker: {err}")));
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
        .map_err(|e| format!("could not start the SPICE runtime: {e}"))?;

    runtime.block_on(async move {
        evt_tx
            .send(SessionEvent::Status(format!("Connecting to {}…", connection.address())))
            .ok();

        let mut client = SpiceClientShared::new(connection.host.clone(), connection.port);
        if let Some(password) = &connection.password {
            client.set_password(password.clone()).await;
        }
        client.connect().await.map_err(|e| format!("SPICE connection failed: {e}"))?;

        let client = Arc::new(client);
        let event_loop_client = Arc::clone(&client);
        let loop_tx = evt_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = event_loop_client.start_event_loop().await {
                loop_tx.send(SessionEvent::Error(format!("SPICE event loop stopped: {err}"))).ok();
                loop_tx.send(SessionEvent::Disconnected).ok();
            }
        });

        let inputs = find_channel(&client, Channel::Inputs).await
            .ok_or("the SPICE server exposed no inputs channel")?;
        let display = find_channel(&client, Channel::Display).await
            .ok_or("the SPICE server exposed no display channel")?;

        evt_tx.send(SessionEvent::Status("Waiting for the first frame…".into())).ok();

        let video = client.get_video_output().await;
        let mut last_count = video.get_frame_count().await;
        let mut announced = false;
        let mut down_buttons = 0u8;

        loop {
            while let Ok(command) = cmd_rx.try_recv() {
                if !handle_command(&client, inputs, &mut down_buttons, command).await {
                    let _ = client.disconnect().await;
                    evt_tx.send(SessionEvent::Disconnected).ok();
                    return Ok(());
                }
            }

            let count = video.get_frame_count().await;
            if count != last_count {
                last_count = count;
                if let Some(surface) = client.get_display_surface(display).await
                    && let Some(patch) = surface_patch(surface.width, surface.height, &surface.data)
                {
                    if !announced {
                        announced = true;
                        evt_tx
                            .send(SessionEvent::Connected {
                                name: connection.display_name().to_string(),
                                width: patch.width,
                                height: patch.height,
                            })
                            .ok();
                    }
                    evt_tx.send(SessionEvent::Frame(patch)).ok();
                }
            }

            tokio::time::sleep(POLL).await;
        }
    })
}

enum Channel {
    Display,
    Inputs,
}

/// Probe the low channel ids for the one the server bound to this channel type.
///
/// `spice-client` assigns ids from the server's channel list at connect time
/// and never exposes them; both probes are side-effect free apart from a
/// pointer report at the origin.
async fn find_channel(client: &SpiceClientShared, want: Channel) -> Option<u8> {
    const IDS: std::ops::RangeInclusive<u8> = 0..=15;
    for id in IDS {
        match want {
            Channel::Display => {
                if client.get_display_surface(id).await.is_some() {
                    return Some(id);
                }
            }
            Channel::Inputs => {
                if client.send_mouse_motion(id, 0, 0).await.is_ok() {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// Returns `false` when the session should end.
async fn handle_command(
    client: &SpiceClientShared,
    inputs: u8,
    down_buttons: &mut u8,
    command: Cmd,
) -> bool {
    match command {
        Cmd::Pointer { x, y, buttons, .. } => {
            if client.send_mouse_motion(inputs, i32::from(x), i32::from(y)).await.is_err() {
                return false;
            }
            let changed = *down_buttons ^ buttons;
            for (bit, button) in [
                (buttons::LEFT, MouseButton::Left),
                (buttons::MIDDLE, MouseButton::Middle),
                (buttons::RIGHT, MouseButton::Right),
            ] {
                if changed & bit == 0 {
                    continue;
                }
                let pressed = buttons & bit != 0;
                if client.send_mouse_button(inputs, button, pressed).await.is_err() {
                    return false;
                }
            }
            *down_buttons = buttons;
            true
        }
        Cmd::Scroll { dx, dy, .. } => {
            client.send_mouse_wheel(inputs, i32::from(dx), i32::from(dy)).await.is_ok()
        }
        Cmd::Key(key) => {
            // SPICE carries extended scancodes as the 0xE0 prefix in the high byte.
            let scancode = if key.extended {
                (0xE0u32 << 8) | key.scancode
            } else {
                key.scancode
            };
            let result = if key.down {
                client.send_key_down(inputs, scancode).await
            } else {
                client.send_key_up(inputs, scancode).await
            };
            result.is_ok()
        }
        Cmd::ClipboardText(_) | Cmd::Resize { .. } => true,
        Cmd::Close => false,
    }
}

/// Turn a SPICE display surface into a full-framebuffer patch.
///
/// The display channel publishes RGBA; anything else is rejected rather than
/// guessed at.
fn surface_patch(width: u32, height: u32, data: &[u8]) -> Option<FramePatch> {
    let expected = width as usize * height as usize * 4;
    if width == 0 || height == 0 || data.len() < expected {
        return None;
    }
    Some(FramePatch::full(width, height, data[..expected].to_vec()))
}
