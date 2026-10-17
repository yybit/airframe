//! AirFrame: a native remote-desktop client for VNC, RDP and SPICE.
//!
//! The crate is the app's library half so the Android build can link it as a
//! `cdylib` and provide the `android_main` entry point; `main.rs` is the thin
//! desktop binary. Both share [`run`] and the window setup here.

mod app;
mod editor;
mod keymap;
mod session;
mod sidebar;
mod viewport;

#[cfg(test)]
mod ui_test;

use airframe_core::{Store, StoreError};
use app::App;

/// Run the app with an already-loaded [`Store`].
pub fn run(
    store: Store,
    startup_error: Option<StoreError>,
    options: eframe::NativeOptions,
) -> eframe::Result {
    eframe::run_native(
        "AirFrame",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(store, startup_error)))),
    )
}

/// Desktop entry point used by the `airframe` binary: logging to stderr, the
/// store in the platform config directory, then the run loop.
///
/// Android uses [`run_on_android`] instead — the two differ only in where
/// logging goes and where the store lives.
pub fn run_desktop() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (store, error) = Store::load();
    if let Some(error) = &error {
        log::error!("{error}");
    }

    run(store, error, desktop_options())
}

/// The window AirFrame opens on the desktop platforms.
pub fn desktop_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AirFrame")
            .with_app_id("airframe")
            .with_inner_size([1320.0, 840.0])
            .with_min_inner_size([880.0, 560.0])
            .with_icon(airframe_ui::logo::raster(64, 0.0)),
        ..Default::default()
    }
}

/// Android entry point, called by the `airframe-android` crate's `android_main`.
///
/// Android has no command line, so this sets up logging to logcat and keeps the
/// store inside the app's private data directory instead of the desktop config
/// root (which Android does not provide).
#[cfg(target_os = "android")]
pub fn run_on_android(app: winit::platform::android::activity::AndroidApp) -> eframe::Result {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let (store, error) = match app.internal_data_path() {
        Some(dir) => Store::load_from(dir.join("airframe.json")),
        None => Store::load(),
    };
    if let Some(error) = &error {
        log::error!("{error}");
    }

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    run(store, error, options)
}
