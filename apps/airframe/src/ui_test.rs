//! Headless UI tests.
//!
//! They drive the real [`App`] through `egui_kittest`, so every widget is laid
//! out and painted for real — no window, no display server, no window manager.
//! Each test also writes a PNG snapshot, which is the visual record of the
//! design: `tests/snapshots/*.png` in this crate.
//!
//! Refresh them after an intentional change with:
//!
//! ```text
//! UPDATE_SNAPSHOTS=1 cargo test -p airframe
//! ```

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;

use airframe_core::{Connection, Lang, Protocol, RdpOptions, Store, Theme};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use uuid::Uuid;

use crate::app::{Action, App};
use crate::editor::Editor;

const BUILD_VNC: Uuid = Uuid::from_u128(0x1111_1111_1111_1111_1111_1111_1111_1111);
const DESIGN_RDP: Uuid = Uuid::from_u128(0x2222_2222_2222_2222_2222_2222_2222_2222);
const HOME_SPICE: Uuid = Uuid::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333);

/// A `Store` that never touches the real config file.
///
/// The path is a fixed name inside the temp directory — not random — because the
/// sidebar footer prints it, and the snapshots have to be byte-reproducible.
/// Nothing in these tests saves the store.
fn store(theme: Theme, connections: Vec<Connection>) -> Store {
    let scratch: PathBuf = std::env::temp_dir().join("airframe-ui-test-config.json");
    let (mut store, error) = Store::load_from(scratch);
    assert!(error.is_none(), "scratch store should load cleanly");
    store.data.settings.theme = theme;
    store.data.settings.language = Lang::En;
    store.data.connections = connections;
    store
}

fn app(theme: Theme, connections: Vec<Connection>) -> App {
    App::new(store(theme, connections), None)
}

fn connection(
    id: Uuid,
    name: &str,
    protocol: Protocol,
    host: &str,
    group: &str,
) -> Connection {
    Connection {
        id,
        name: name.to_owned(),
        protocol,
        host: host.to_owned(),
        port: protocol.default_port(),
        username: if protocol == Protocol::Rdp { "ada".to_owned() } else { "root".to_owned() },
        domain: if protocol == Protocol::Rdp { "ACME".to_owned() } else { String::new() },
        group: group.to_owned(),
        view_only: protocol == Protocol::Vnc,
        remember_password: false,
        password: None,
        vnc: Default::default(),
        rdp: RdpOptions::default(),
    }
}

/// The three saved connections the sidebar tests are built around.
fn sample_connections() -> Vec<Connection> {
    let mut design = connection(
        DESIGN_RDP,
        "Design workstation",
        Protocol::Rdp,
        "10.20.0.9",
        "Work",
    );
    design.remember_password = true;
    design.password = Some("hunter2".to_owned());

    let mut lab = connection(HOME_SPICE, "Home lab", Protocol::Spice, "192.168.1.20", "Home");
    lab.username = "ada".to_owned();

    vec![
        connection(BUILD_VNC, "Build server", Protocol::Vnc, "10.20.0.5", "Work"),
        design,
        lab,
    ]
}

/// A harness sized like the real window, with wgpu rendering for snapshots.
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size([1280.0, 800.0])
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app)
}

#[test]
fn onboarding_dark() {
    let mut harness = harness(app(Theme::Dark, Vec::new()));
    harness.run();
    harness.get_by_label_contains("One native remote desktop");
    harness.snapshot("onboarding-dark");
}

#[test]
fn onboarding_light() {
    let mut harness = harness(app(Theme::Light, Vec::new()));
    harness.run();
    harness.snapshot("onboarding-light");
}

#[test]
fn sidebar_search_filters_the_list() {
    let mut app = app(Theme::Light, sample_connections());
    app.search = "lab".to_owned();
    let mut harness = harness(app);
    harness.run();

    harness.get_by_label_contains("Home lab");
    assert!(
        harness.query_by_label_contains("Build server").is_none(),
        "the search term should hide non-matching connections"
    );
    harness.snapshot("sidebar-search-light");
}

#[test]
fn sidebar_lists_every_saved_connection() {
    let mut harness = harness(app(Theme::Dark, sample_connections()));
    harness.run();
    harness.get_by_label_contains("Build server");
    harness.get_by_label_contains("Design workstation");
    harness.get_by_label_contains("Home lab");
    harness.snapshot("sidebar-dark");
}

#[test]
fn editor_dialog_renders() {
    let mut app = app(Theme::Dark, sample_connections());
    let saved = app.store.data.connections[1].clone();
    app.editor = Some(Editor::edit(&saved));
    let mut harness = harness(app);
    harness.run();
    harness.snapshot("editor-dialog");
}

#[test]
fn new_connection_dialog_renders() {
    let mut app = app(Theme::Light, sample_connections());
    app.editor = Some(Editor::new());
    let mut harness = harness(app);
    harness.run();
    harness.snapshot("editor-dialog-light");
}

#[test]
fn settings_window_renders() {
    let mut app = app(Theme::Dark, sample_connections());
    app.settings_open = true;
    let mut harness = harness(app);
    harness.run();
    harness.snapshot("settings-window");
}

#[test]
fn sidebar_chinese() {
    let mut app = app(Theme::Dark, sample_connections());
    app.store.data.settings.language = Lang::Zh;
    let mut harness = harness(app);
    harness.run();
    harness.snapshot("sidebar-chinese");
}

#[test]
fn settings_window_chinese() {
    let mut app = app(Theme::Dark, sample_connections());
    app.store.data.settings.language = Lang::Zh;
    app.settings_open = true;
    let mut harness = harness(app);
    harness.run();
    harness.snapshot("settings-window-chinese");
}

#[test]
fn send_text_window_renders() {
    let mut app = app(Theme::Dark, sample_connections());
    app.send_text = Some(":put [/system resource get uptime]\n".to_owned());
    let mut harness = harness(app);
    harness.run_steps(2);
    harness.snapshot("send-text-window");
}

#[test]
fn the_send_text_action_opens_the_window() {
    let mut app = app(Theme::Dark, sample_connections());
    app.actions.push(Action::SendText);
    let mut harness = harness(app);
    harness.run_steps(1);
    assert_eq!(harness.state().send_text.as_deref(), Some(""));
}

/// A socket that accepts and then says nothing: the VNC handshake stalls, so the
/// tab stays in its connecting state forever and the snapshot is stable.
///
/// The port is fixed rather than ephemeral because the address is printed in
/// both the status line and the sidebar subtitle, and a snapshot must be
/// reproducible. Nothing else may be listening on it.
const BLACKHOLE_PORT: u16 = 59877;

fn blackhole() -> TcpListener {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, BLACKHOLE_PORT))).unwrap_or_else(
        |error| panic!("this UI snapshot needs 127.0.0.1:{BLACKHOLE_PORT} free: {error}"),
    )
}

#[test]
fn the_workspace_shows_a_session_while_it_connects() {
    let _listener = blackhole();
    let address = format!("127.0.0.1:{BLACKHOLE_PORT}");
    let mut host = connection(BUILD_VNC, "Build server", Protocol::Vnc, "127.0.0.1", "Work");
    host.port = BLACKHOLE_PORT;

    let mut app = app(Theme::Dark, vec![host]);
    app.actions.push(Action::Connect(BUILD_VNC));

    let mut harness = harness(app);
    // Frame one drains the action, which opens the tab and spawns its worker.
    harness.run_steps(1);
    // Let the worker report the connection attempt. Sleeping instead of stepping
    // keeps the frame count — and so the spinner's phase — reproducible.
    std::thread::sleep(std::time::Duration::from_millis(250));
    harness.run_steps(2);

    let status = harness.state().tabs[0].session.status.clone();
    assert!(status.contains(&address), "unexpected session status: {status}");
    assert_eq!(harness.state().tabs.len(), 1, "Connect should open a session");
    assert_eq!(
        harness.get_all_by_label_contains("Build server").count(),
        1,
        "the sidebar row is the only place a session's name appears now"
    );
    harness.snapshot("session-connecting");
}

/// The stored connections, in list order — the observable side of deleting.
fn names(app: &App) -> Vec<&str> {
    app.store.data.connections.iter().map(|c| c.name.as_str()).collect()
}

/// Click a primary mouse button at an arbitrary point, as a user would.
fn click_at(harness: &mut Harness<'static, App>, at: egui::Pos2) {
    let button = egui::PointerButton::Primary;
    let modifiers = egui::Modifiers::default();
    harness.input_mut().events.push(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.input_mut().events.push(egui::Event::PointerButton { pos: at, button, pressed, modifiers });
        harness.run();
    }
}

#[test]
fn the_row_menu_deletes_a_connection() {
    let mut harness = harness(app(Theme::Dark, sample_connections()));
    harness.run();

    harness.get_by_label_contains("Design workstation").click_secondary();
    harness.run();
    harness.get_by_label("Delete").click();
    harness.run();
    harness.get_by_label("Delete").click();
    // Deleting raises a toast, which asks for another repaint every 120 ms, so
    // the harness cannot be asked to run to a steady state here.
    harness.run_steps(2);

    assert_eq!(names(harness.state()), ["Build server", "Home lab"]);
}

#[test]
fn the_row_actions_delete_a_connection() {
    let mut harness = harness(app(Theme::Dark, sample_connections()));
    harness.run();

    // The pencil and the trash sit at the row's trailing edge, and only appear
    // once the pointer is over the row.
    let row = harness.get_by_label_contains("Design workstation").rect();
    click_at(&mut harness, egui::pos2(row.right() - 42.0, row.center().y));
    harness.get_by_label("Delete").click();
    harness.run_steps(2);

    assert_eq!(names(harness.state()), ["Build server", "Home lab"]);
}

/// The selected row is the one place the row actions are always visible, so
/// they are captured in a golden image rather than only existing on hover.
#[test]
fn sidebar_selected_row_shows_its_actions() {
    let connections = sample_connections();
    let selected = connections[1].id;
    let mut harness = harness(app(Theme::Dark, connections));
    harness.state_mut().selection = Some(selected);
    harness.run();
    harness.snapshot("sidebar-selected-row");
}
