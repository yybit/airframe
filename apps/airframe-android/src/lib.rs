//! Android entry point for AirFrame.
//!
//! `android-activity` looks up the exported `android_main` symbol when the
//! native activity starts. Everything else — the store, logging and the eframe
//! run loop — lives in the `airframe` library so the desktop and Android builds
//! share one app.

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    airframe::run_on_android(app).expect("AirFrame failed to start");
}
