//! The AirFrame mark, rendered from `assets/airframe-logo.svg`.
//!
//! The SVG is the design source and is embedded at build time, so replacing
//! that file and rebuilding updates the header mark, the window icon and the
//! macOS bundle's `.icns` together: [`paint`] draws the mark into the interface
//! at whatever size egui asks for, and [`raster`] bakes the RGBA bitmaps the
//! icon formats need. Both rasterise with `resvg`, so the SVG's own semantics —
//! gradients, stroke caps and joins — are what lands on screen.
//!
//! The mark is drawn as shapes. `resvg` is built here without its text support,
//! so a `<text>` element would not appear; lettering belongs in the logo as
//! outlines. A malformed SVG leaves the mark blank rather than panicking, which
//! the tests in this module catch.

use egui::{
    Color32, ColorImage, Context, IconData, Id, Pos2, Rect, TextureHandle, TextureOptions, Ui, Vec2,
};

/// The SVG, embedded so a build cannot draw one image and ship another.
const SVG: &str = include_str!("../../../assets/airframe-logo.svg");

/// Transparent margin the Dock wants around an app icon, matching Apple's icon
/// grid (the rounded square covers roughly the middle 80% of the canvas).
pub const MACOS_BUNDLE_MARGIN: f32 = 0.10;

/// Rasterise the mark as `size`×`size` RGBA8, premultiplied the way epaint
/// wants it (which is also what `resvg` produces).
///
/// `margin` is the transparent border on each side, as a fraction of the
/// canvas: 0 for a window icon, which should be edge to edge, and about
/// [`MACOS_BUNDLE_MARGIN`] for the Dock, where macOS reserves a margin around
/// an app icon's rounded square.
pub fn raster(size: u32, margin: f32) -> IconData {
    let blank = IconData { rgba: Vec::new(), width: 0, height: 0 };
    let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(size, size) else {
        return blank;
    };
    let Ok(tree) = resvg::usvg::Tree::from_str(SVG, &resvg::usvg::Options::default()) else {
        log::error!("assets/airframe-logo.svg does not parse");
        return blank;
    };

    let art = tree.size();
    let side = size as f32 * (1.0 - 2.0 * margin);
    // Fit the artwork into the canvas and centre it: square today, but nothing
    // in the SVG says it has to stay that way.
    let scale = (side / art.width()).min(side / art.height());
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(
        (size as f32 - scale * art.width()) * 0.5,
        (size as f32 - scale * art.height()) * 0.5,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    IconData { rgba: pixmap.take(), width: size, height: size }
}

/// Draw the mark inside `rect`, treating it as a square.
pub fn paint(ui: &Ui, rect: Rect) {
    let side = rect.width().min(rect.height());
    if side <= 0.0 {
        return;
    }
    // Rasterise at the size the display actually gets, so the mark stays sharp
    // on HiDPI screens and under egui's zoom. One texture per size, kept alive
    // by the cache entry; handles are refcounted, so cloning one out is cheap.
    // The cache has to be read without holding its lock while the texture is
    // uploaded, because `load_texture` reads through the same lock.
    let pixels = (side * ui.ctx().pixels_per_point()).ceil().max(1.0) as u32;
    let id = Id::new(("airframe-logo", pixels));
    let mark = ui.ctx().data(|data| data.get_temp::<TextureHandle>(id));
    let mark = match mark {
        Some(mark) => mark,
        None => {
            let mark = texture(ui.ctx(), pixels);
            ui.ctx().data_mut(|data| data.insert_temp(id, mark.clone()));
            mark
        }
    };

    ui.painter().image(
        mark.id(),
        Rect::from_center_size(rect.center(), Vec2::splat(side)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// Upload the mark, `pixels`×`pixels`, as a texture.
fn texture(ctx: &Context, pixels: u32) -> TextureHandle {
    let icon = raster(pixels, 0.0);
    ctx.load_texture(
        "airframe-logo",
        ColorImage::from_rgba_premultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        ),
        TextureOptions::LINEAR,
    )
}

/// Draw the mark inside `rect`, treating it as a square.
#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(icon: &IconData, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * icon.width + x) * 4) as usize;
        [icon.rgba[index], icon.rgba[index + 1], icon.rgba[index + 2], icon.rgba[index + 3]]
    }

    #[test]
    fn the_mark_covers_the_canvas_but_not_the_margin() {
        let edge_to_edge = raster(64, 0.0);
        assert_eq!(pixel(&edge_to_edge, 0, 0)[3], 0, "the corner is outside the squircle");
        assert_eq!(pixel(&edge_to_edge, 32, 32)[3], 255, "the mark's middle is opaque");

        let dock = raster(64, MACOS_BUNDLE_MARGIN);
        assert_eq!(pixel(&dock, 0, 0)[3], 0, "the margin stays transparent");
        assert_eq!(pixel(&dock, 32, 32)[3], 255, "the mark still fills its own square");
    }

    #[test]
    fn the_mark_is_the_svg() {
        let icon = raster(128, 0.0);
        let base = pixel(&icon, 64, 12);
        assert_eq!(base[3], 255);
        assert!(base[0] < 0x20 && base[2] < 0x40, "the base is navy, got {base:?}");

        // The frame's top edge, lit by the SVG's gradient.
        let frame = pixel(&icon, 64, 38);
        assert!(frame[2] > 0x80 && frame[0] > 0x30, "the frame is lit, got {frame:?}");

        // The node the stream ends on, at (392, 296) in the SVG.
        let node = pixel(&icon, 98, 74);
        assert!(node[2] > 200 && node[0] < 0x40, "the node is cyan, got {node:?}");
    }

    #[test]
    fn painting_the_mark_records_an_image() {
        let context = egui::Context::default();
        let output = context.run_ui(Default::default(), |ui| {
            paint(ui, Rect::from_min_size(Pos2::ZERO, Vec2::splat(48.0)));
        });
        let recorded = output.shapes.len();
        output.drop_without_applying_deltas();
        assert!(recorded >= 1, "the mark, got {recorded}");
    }
}
