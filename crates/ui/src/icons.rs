//! Icon set drawn straight onto the painter.
//!
//! Keeping the glyphs as vector strokes means the app ships no icon font and
//! every mark scales cleanly with the display's pixel density.

use std::f32::consts::TAU;

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Plus,
    Close,
    Pencil,
    Trash,
    Refresh,
    Search,
    ChevronDown,
    ChevronRight,
    Expand,
    Shrink,
    Power,
    Check,
    Sun,
    Moon,
    Warning,
    Info,
    Gear,
    /// Scale the desktop to the window.
    Fit,
    /// One image pixel per screen pixel.
    Pixel,
    /// Fill the window, ignoring the aspect ratio.
    Stretch,
    /// Watch without sending input.
    Eye,
    /// Save a picture of the desktop.
    Camera,
    /// Type text into the remote session.
    Keyboard,
}

/// Paint `icon` to fill `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let rect = rect.shrink(rect.width() * 0.16);
    let w = rect.width();
    let stroke = Stroke::new((w * 0.13).clamp(1.1, 2.2), color);
    let point = |fx: f32, fy: f32| pos2(rect.left() + w * fx, rect.top() + w * fy);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    // Outline of the box spanned by two corners given in `point` units.
    let outline = |a: (f32, f32), b: (f32, f32)| {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        line(point(x0, y0), point(x1, y0));
        line(point(x1, y0), point(x1, y1));
        line(point(x1, y1), point(x0, y1));
        line(point(x0, y1), point(x0, y0));
    };

    match icon {
        Icon::Plus => {
            line(point(0.5, 0.08), point(0.5, 0.92));
            line(point(0.08, 0.5), point(0.92, 0.5));
        }
        Icon::Close => {
            line(point(0.16, 0.16), point(0.84, 0.84));
            line(point(0.84, 0.16), point(0.16, 0.84));
        }
        Icon::Pencil => {
            line(point(0.12, 0.88), point(0.74, 0.26));
            line(point(0.68, 0.2), point(0.8, 0.32));
            line(point(0.12, 0.88), point(0.3, 0.84));
        }
        Icon::Trash => {
            line(point(0.12, 0.22), point(0.88, 0.22));
            line(point(0.36, 0.22), point(0.4, 0.1));
            line(point(0.6, 0.1), point(0.64, 0.22));
            line(point(0.2, 0.22), point(0.26, 0.9));
            line(point(0.8, 0.22), point(0.74, 0.9));
            line(point(0.26, 0.9), point(0.74, 0.9));
        }
        Icon::Refresh => {
            arc(painter, rect.center(), w * 0.38, 0.62, TAU + 0.15, stroke);
            let tip = arc_point(rect.center(), w * 0.38, 0.15);
            painter.add(Shape::convex_polygon(
                vec![
                    tip + vec2(-w * 0.1, -w * 0.02),
                    tip + vec2(w * 0.1, w * 0.02),
                    tip + vec2(-w * 0.04, w * 0.16),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Search => {
            painter.circle_stroke(rect.center() + vec2(-w * 0.08, -w * 0.08), w * 0.28, stroke);
            line(point(0.64, 0.64), point(0.9, 0.9));
        }
        Icon::ChevronDown => {
            line(point(0.2, 0.38), point(0.5, 0.68));
            line(point(0.5, 0.68), point(0.8, 0.38));
        }
        Icon::ChevronRight => {
            line(point(0.38, 0.2), point(0.68, 0.5));
            line(point(0.68, 0.5), point(0.38, 0.8));
        }
        Icon::Expand => {
            line(point(0.08, 0.3), point(0.08, 0.08));
            line(point(0.08, 0.08), point(0.3, 0.08));
            line(point(0.7, 0.08), point(0.92, 0.08));
            line(point(0.92, 0.08), point(0.92, 0.3));
            line(point(0.92, 0.7), point(0.92, 0.92));
            line(point(0.92, 0.92), point(0.7, 0.92));
            line(point(0.3, 0.92), point(0.08, 0.92));
            line(point(0.08, 0.92), point(0.08, 0.7));
        }
        Icon::Shrink => {
            line(point(0.34, 0.08), point(0.34, 0.34));
            line(point(0.08, 0.34), point(0.34, 0.34));
            line(point(0.66, 0.08), point(0.66, 0.34));
            line(point(0.92, 0.34), point(0.66, 0.34));
            line(point(0.34, 0.92), point(0.34, 0.66));
            line(point(0.08, 0.66), point(0.34, 0.66));
            line(point(0.92, 0.66), point(0.66, 0.66));
            line(point(0.66, 0.92), point(0.66, 0.66));
        }
        Icon::Power => {
            arc(painter, rect.center() + vec2(0.0, w * 0.06), w * 0.38, -0.9, TAU - 1.3, stroke);
            line(point(0.5, 0.08), point(0.5, 0.46));
        }
        Icon::Check => {
            line(point(0.16, 0.54), point(0.4, 0.78));
            line(point(0.4, 0.78), point(0.86, 0.24));
        }
        Icon::Sun => {
            painter.circle_stroke(rect.center(), w * 0.22, stroke);
            for i in 0..8 {
                let angle = i as f32 * TAU / 8.0;
                let inner = arc_point(rect.center(), w * 0.32, angle);
                let outer = arc_point(rect.center(), w * 0.46, angle);
                line(inner, outer);
            }
        }
        Icon::Moon => {
            arc(painter, rect.center(), w * 0.4, 0.7, TAU - 0.7, stroke);
            arc(painter, rect.center() + vec2(w * 0.28, 0.0), w * 0.42, 1.9, TAU - 1.9, stroke);
        }
        Icon::Warning => {
            let points = vec![point(0.5, 0.08), point(0.94, 0.88), point(0.06, 0.88)];
            painter.add(Shape::closed_line(points, stroke));
            line(point(0.5, 0.38), point(0.5, 0.62));
            painter.circle_filled(point(0.5, 0.76), w * 0.06, color);
        }
        Icon::Info => {
            painter.circle_stroke(rect.center(), w * 0.42, stroke);
            line(point(0.5, 0.44), point(0.5, 0.72));
            painter.circle_filled(point(0.5, 0.28), w * 0.06, color);
        }
        Icon::Gear => {
            painter.circle_stroke(rect.center(), w * 0.19, stroke);
            for i in 0..8 {
                let angle = i as f32 * TAU / 8.0 + TAU / 16.0;
                line(
                    arc_point(rect.center(), w * 0.3, angle),
                    arc_point(rect.center(), w * 0.45, angle),
                );
            }
        }
        Icon::Fit => {
            outline((0.06, 0.22), (0.94, 0.78));
            outline((0.34, 0.4), (0.66, 0.6));
        }
        Icon::Pixel => {
            // Four squares: one image pixel per screen pixel.
            for (x, y) in [(0.18, 0.18), (0.54, 0.18), (0.18, 0.54), (0.54, 0.54)] {
                painter.rect_filled(
                    Rect::from_min_max(point(x, y), point(x + 0.28, y + 0.28)),
                    CornerRadius::same(1),
                    color,
                );
            }
        }
        Icon::Stretch => {
            outline((0.06, 0.28), (0.94, 0.72));
            line(point(0.26, 0.5), point(0.74, 0.5));
            line(point(0.26, 0.5), point(0.38, 0.4));
            line(point(0.26, 0.5), point(0.38, 0.6));
            line(point(0.74, 0.5), point(0.62, 0.4));
            line(point(0.74, 0.5), point(0.62, 0.6));
        }
        Icon::Eye => {
            line(point(0.06, 0.5), point(0.28, 0.28));
            line(point(0.28, 0.28), point(0.72, 0.28));
            line(point(0.72, 0.28), point(0.94, 0.5));
            line(point(0.94, 0.5), point(0.72, 0.72));
            line(point(0.72, 0.72), point(0.28, 0.72));
            line(point(0.28, 0.72), point(0.06, 0.5));
            painter.circle_filled(rect.center(), w * 0.14, color);
        }
        Icon::Camera => {
            line(point(0.34, 0.2), point(0.34, 0.32));
            line(point(0.34, 0.2), point(0.62, 0.2));
            line(point(0.62, 0.2), point(0.62, 0.32));
            outline((0.06, 0.32), (0.94, 0.86));
            painter.circle_stroke(pos2(rect.center().x, rect.top() + w * 0.59), w * 0.17, stroke);
        }
        Icon::Keyboard => {
            outline((0.04, 0.24), (0.96, 0.76));
            for i in 0..3 {
                let x = 0.18 + i as f32 * 0.32;
                painter.rect_filled(
                    Rect::from_min_max(point(x, 0.36), point(x + 0.12, 0.46)),
                    CornerRadius::same(1),
                    color,
                );
            }
            line(point(0.36, 0.62), point(0.64, 0.62));
        }
    }
}

fn arc_point(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    pos2(center.x + radius * angle.cos(), center.y + radius * angle.sin())
}

fn arc(painter: &Painter, center: Pos2, radius: f32, from: f32, to: f32, stroke: Stroke) {
    const STEPS: usize = 28;
    let points = (0..=STEPS)
        .map(|i| {
            let t = from + (to - from) * (i as f32 / STEPS as f32);
            arc_point(center, radius, t)
        })
        .collect();
    painter.add(Shape::line(points, stroke));
}
