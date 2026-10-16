//! End-to-end check against a live RFB server.
//!
//! Start one with, for example:
//!
//! ```sh
//! qemu-system-x86_64 -display none -vnc 127.0.0.1:9 -m 128 -nodefaults
//! AIRFRAME_VNC_TEST=127.0.0.1:5909 cargo test -p airframe-vnc --test live -- --nocapture
//! ```
//!
//! Without `AIRFRAME_VNC_TEST` the test is a no-op, so the suite stays green
//! offline.

use std::time::{Duration, Instant};

use airframe_core::{Connection, Protocol, SessionEvent};

#[test]
fn live_session_against_a_real_server() {
    let Ok(addr) = std::env::var("AIRFRAME_VNC_TEST") else {
        eprintln!("AIRFRAME_VNC_TEST is not set; skipping the live VNC test");
        return;
    };
    let (host, port) = addr.rsplit_once(':').expect("host:port");
    let mut connection = Connection::new("live", Protocol::Vnc, host);
    connection.port = port.parse().expect("port");

    let handle = airframe_vnc::start(&connection);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut connected = None;
    let mut pixels = 0usize;
    let mut patches = 0usize;

    while Instant::now() < deadline {
        match handle.events.recv_timeout(Duration::from_millis(500)) {
            Ok(SessionEvent::Connected { width, height, .. }) => {
                connected = Some((width, height));
            }
            Ok(SessionEvent::Frame(patch)) => {
                patches += 1;
                pixels += patch.rgba.len() / 4;
                // Every pixel the server sends must be an opaque colour.
                assert!(patch.rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255));
            }
            Ok(SessionEvent::Error(message)) => panic!("session error: {message}"),
            Ok(SessionEvent::Disconnected) => break,
            Ok(_) => {}
            Err(_) => {}
        }
        if patches > 4 {
            break;
        }
    }

    assert!(connected.is_some(), "no Connected event arrived");
    assert!(patches > 0, "no framebuffer update arrived");
    println!("received {patches} patches covering {pixels} pixels");
}
