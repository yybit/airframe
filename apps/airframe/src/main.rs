//! AirFrame desktop entry point. The app itself lives in the library half so
//! the Android build can reuse it; see `lib.rs`.

fn main() -> eframe::Result {
    airframe::run_desktop()
}
