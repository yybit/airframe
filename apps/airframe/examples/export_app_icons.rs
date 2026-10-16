//! Export AirFrame icons for the Linux and Windows packages:
//!
//! ```text
//! cargo run --release -p airframe --example export_app_icons -- <out-dir>
//! ```
//!
//! Writes `airframe-<size>.png` for the freedesktop hicolor icon theme and
//! `airframe.ico` (PNG-compressed entries, Windows Vista+) for the MSI, all
//! rasterised from the one SVG embedded in `airframe-ui::logo` — the same
//! design source as the window icon and the macOS `.icns`.

use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

/// Sizes embedded in the .ico. 256 is stored as `0` in the ICO directory
/// entry, per the format spec.
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// Sizes written as standalone PNGs (hicolor icons).
const PNG_SIZES: [u32; 2] = [256, 512];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&out)?;

    let mut pngs = Vec::new();
    for size in PNG_SIZES {
        let path = out.join(format!("airframe-{size}.png"));
        write_png(&path, encode_png(airframe_ui::logo::raster(size, 0.0))?);
        println!("wrote {}", path.display());
    }

    for size in ICO_SIZES {
        pngs.push((size, encode_png(airframe_ui::logo::raster(size, 0.0))?));
    }
    let path = out.join("airframe.ico");
    write_ico(&path, &pngs)?;
    println!("wrote {}", path.display());

    Ok(())
}

/// Rasterise one icon and PNG-encode it in memory.
fn encode_png(icon: egui::IconData) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(BufWriter::new(&mut png), icon.width, icon.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&icon.rgba)?;
    Ok(png)
}

fn write_png(path: &Path, bytes: Vec<u8>) {
    if let Err(err) = fs::write(path, bytes) {
        eprintln!("warning: could not write {}: {err}", path.display());
    }
}

/// Wrap PNG images in an .ico container (PNG-compressed entries).
fn write_ico(path: &Path, entries: &[(u32, Vec<u8>)]) -> std::io::Result<()> {
    let mut out = Vec::new();
    // ICONDIR: reserved, type (1 = icon), count.
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let mut offset = (6 + 16 * entries.len()) as u32;
    for &(size, ref png) in entries {
        // Width/height: 256 is encoded as 0.
        let side = if size >= 256 { 0u8 } else { size as u8 };
        out.push(side);
        out.push(side);
        out.push(0); // palette colours
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in entries {
        out.extend_from_slice(png);
    }
    fs::write(path, out)
}
