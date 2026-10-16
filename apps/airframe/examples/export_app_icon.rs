//! Writes the app icon as a macOS `.icns` for `packaging/macos/package.sh`:
//!
//!     cargo run --release --example export_app_icon -- packaging/macos/AirFrame.icns
//!
//! The rasteriser lives in `airframe-ui` and is the same one the window icon
//! uses, so the bundle icon and the window icon cannot drift apart.

use std::fs;
use std::io::Write as _;

use airframe_ui::logo;

/// `(ICNS type code, side length in pixels)`. macOS has understood PNG
/// payloads since 10.7.
const VARIANTS: &[(&[u8; 4], u32)] = &[
    (b"icp4", 16),
    (b"icp5", 32),
    (b"icp6", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        return Err("usage: export_app_icon <out.icns>".into());
    };

    let mut entries = Vec::new();
    for (kind, size) in VARIANTS {
        let icon = logo::raster(*size, logo::MACOS_BUNDLE_MARGIN);
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, icon.width, icon.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&icon.rgba)?;
        entries.push((*kind, png));
    }

    // An ICNS file is a header followed by type- and size-tagged chunks.
    let length: usize = 8 + entries.iter().map(|(_, png)| 8 + png.len()).sum::<usize>();
    let mut icns = Vec::with_capacity(length);
    icns.write_all(b"icns")?;
    icns.write_all(&(length as u32).to_be_bytes())?;
    for (kind, png) in &entries {
        icns.write_all(*kind)?;
        icns.write_all(&((png.len() + 8) as u32).to_be_bytes())?;
        icns.write_all(png)?;
    }

    fs::write(&path, &icns)?;
    println!("wrote {path}: {} bytes, {} sizes", icns.len(), entries.len());
    Ok(())
}
