//! Small shared widgets: icon buttons, protocol badges and status pills.

use airframe_core::{Protocol, Theme};
use egui::{
    Color32, CornerRadius, Response, Sense, TextStyle, Ui, Vec2, WidgetText, vec2,
};

use crate::{icons, icons::Icon, theme};

/// A borderless square button that paints an [`Icon`].
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, tint: Color32, tooltip: &str) -> Response {
    icon_toggle(ui, icon, size, tint, false, tooltip)
}

/// An icon button that stays lit while its mode is on, for view options such as
/// the zoom mode or view-only.
pub fn icon_toggle(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    tint: Color32,
    on: bool,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let visuals = ui.style().interact(&response);
    let p = theme::palette(if ui.visuals().dark_mode { Theme::Dark } else { Theme::Light });
    if on {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), p.accent.gamma_multiply(0.32));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), visuals.weak_bg_fill);
    }
    let color = if on {
        p.accent
    } else if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        tint
    };
    icons::paint(ui.painter(), rect, icon, color);
    if tooltip.is_empty() { response } else { response.on_hover_text(tooltip) }
}

/// A small tinted chip, e.g. `VNC` or `NLA`.
pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    chip(ui, text, color, 0.16, color)
}

/// A chip filled with its own colour, for primary labels.
pub fn solid_badge(ui: &mut Ui, text: &str, color: Color32, on_color: Color32) -> Response {
    chip(ui, text, color, 1.0, on_color)
}

fn chip(ui: &mut Ui, text: &str, color: Color32, fill_alpha: f32, text_color: Color32) -> Response {
    let pad = vec2(6.0, 2.5);
    let galley = badge_galley(ui, text, text_color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    let fill = if fill_alpha >= 1.0 { color } else { color.gamma_multiply(fill_alpha) };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().galley(rect.min + pad, galley, text_color);
    response
}

/// How much room a badge carrying `text` takes up.
///
/// A row of badges has to be measured before it is placed, otherwise there is no
/// way to centre it: a bare [`Ui::horizontal`] claims the whole width, so a
/// [`Ui::vertical_centered`] parent has nothing left to centre.
pub fn badge_size(ui: &Ui, text: &str) -> Vec2 {
    badge_galley(ui, text, Color32::WHITE).size() + vec2(6.0, 2.5) * 2.0
}

fn badge_galley(ui: &Ui, text: &str, color: Color32) -> std::sync::Arc<egui::Galley> {
    let font = TextStyle::Small.resolve(ui.style());
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

/// The protocol's mark: a filled dot in its house colour.
pub fn protocol_dot(ui: &mut Ui, protocol: Protocol, radius: f32) -> (egui::Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(radius * 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, theme::protocol_color(protocol));
    (rect, response)
}

/// A connection state marker.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Connecting,
    Live,
    Failed,
}

impl State {
    pub fn color(self, theme: Theme) -> Color32 {
        let p = theme::palette(theme);
        match self {
            State::Idle => p.text_faint,
            State::Connecting => p.warn,
            State::Live => p.success,
            State::Failed => p.danger,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            State::Idle => "Not connected",
            State::Connecting => "Connecting",
            State::Live => "Connected",
            State::Failed => "Failed",
        }
    }
}

/// A labelled status pill with a coloured dot.
pub fn state_pill(ui: &mut Ui, state: State, theme: Theme, extra: Option<&str>) -> Response {
    let p = theme::palette(theme);
    let color = state.color(theme);
    let text = match extra {
        Some(extra) if !extra.is_empty() => format!("{} · {extra}", state.label()),
        _ => state.label().to_owned(),
    };
    let font = TextStyle::Small.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text, font, p.text_dim);
    let pad = vec2(8.0, 3.0);
    let dot = 7.0;
    let (rect, response) =
        ui.allocate_exact_size(galley.size() + pad * 2.0 + vec2(dot + 5.0, 0.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(9), p.card);
    let dot_center = egui::pos2(rect.left() + pad.x + dot / 2.0, rect.center().y);
    ui.painter().circle_filled(dot_center, dot / 2.0, color);
    ui.painter().galley(
        egui::pos2(dot_center.x + dot / 2.0 + 5.0, rect.center().y - galley.size().y / 2.0),
        galley,
        p.text_dim,
    );
    response
}

/// A text label rendered with an explicit colour.
pub fn label(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    ui.label(WidgetText::from(text.into()).color(color))
}

/// Shorten `text` so it fits `max_width` at `font_size`, marking the cut with
/// an ellipsis.
///
/// This estimates the advance width from the font size rather than measuring,
/// which is enough for the one-line labels in the sidebar and tab strip.
pub fn elide(text: &str, max_width: f32, font_size: f32) -> String {
    let room = (max_width / (font_size * 0.56)).floor().max(3.0) as usize;
    if text.chars().count() <= room {
        return text.to_owned();
    }
    let kept: String = text.chars().take(room - 1).collect();
    format!("{kept}\u{2026}")
}
