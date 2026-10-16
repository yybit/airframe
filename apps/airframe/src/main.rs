//! AirFrame: a native remote-desktop client for VNC, RDP and SPICE.

mod app;
mod editor;
mod keymap;
mod session;
mod sidebar;
mod viewport;

#[cfg(test)]
mod ui_test;

use airframe_core::Store;
use app::App;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (store, error) = Store::load();
    if let Some(error) = &error {
        log::error!("{error}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AirFrame")
            .with_app_id("airframe")
            .with_inner_size([1320.0, 840.0])
            .with_min_inner_size([880.0, 560.0])
            .with_icon(airframe_ui::logo::raster(64, 0.0)),
        ..Default::default()
    };

    eframe::run_native(
        "AirFrame",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(store, error)))),
    )
}
