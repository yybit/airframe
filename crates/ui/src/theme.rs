//! AirFrame's visual language: one palette per theme, plus the egui styling
//! that the whole app is built on.

use airframe_core::{Protocol, Theme};
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke,
    TextStyle, Visuals,
};
use std::sync::Arc;

/// Every colour the interface uses, resolved for one theme.
pub struct Palette {
    /// The window behind everything.
    pub bg: Color32,
    /// Side and top chrome.
    pub panel: Color32,
    /// Cards and list rows.
    pub card: Color32,
    /// A card under the pointer.
    pub card_hover: Color32,
    /// Windows and popups.
    pub elevated: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub warn: Color32,
    pub danger: Color32,
    /// The letterbox around a remote desktop.
    pub viewport: Color32,
    /// The chequerboard-free backdrop behind a disconnected session.
    pub shadow: Color32,
}

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            bg: Color32::from_rgb(0x0B, 0x0E, 0x13),
            panel: Color32::from_rgb(0x12, 0x16, 0x1D),
            card: Color32::from_rgb(0x18, 0x1D, 0x26),
            card_hover: Color32::from_rgb(0x21, 0x28, 0x33),
            elevated: Color32::from_rgb(0x16, 0x1B, 0x23),
            border: Color32::from_rgb(0x24, 0x2B, 0x37),
            border_strong: Color32::from_rgb(0x33, 0x3C, 0x4B),
            text: Color32::from_rgb(0xE8, 0xEC, 0xF3),
            text_dim: Color32::from_rgb(0x9A, 0xA7, 0xBB),
            text_faint: Color32::from_rgb(0x64, 0x70, 0x83),
            accent: Color32::from_rgb(0x4F, 0x8C, 0xFF),
            accent_hover: Color32::from_rgb(0x6C, 0xA0, 0xFF),
            on_accent: Color32::from_rgb(0x0B, 0x0E, 0x13),
            success: Color32::from_rgb(0x3F, 0xB9, 0x50),
            warn: Color32::from_rgb(0xD2, 0x99, 0x22),
            danger: Color32::from_rgb(0xF8, 0x5B, 0x51),
            viewport: Color32::from_rgb(0x05, 0x06, 0x09),
            shadow: Color32::from_black_alpha(90),
        },
        Theme::Light => Palette {
            bg: Color32::from_rgb(0xF4, 0xF6, 0xFA),
            panel: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            card: Color32::from_rgb(0xF7, 0xF9, 0xFC),
            card_hover: Color32::from_rgb(0xEC, 0xF1, 0xF9),
            elevated: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            border: Color32::from_rgb(0xDD, 0xE3, 0xEC),
            border_strong: Color32::from_rgb(0xC3, 0xCC, 0xDA),
            text: Color32::from_rgb(0x16, 0x1B, 0x24),
            text_dim: Color32::from_rgb(0x55, 0x60, 0x72),
            text_faint: Color32::from_rgb(0x93, 0x9E, 0xAF),
            accent: Color32::from_rgb(0x2F, 0x6F, 0xE8),
            accent_hover: Color32::from_rgb(0x1D, 0x5A, 0xCE),
            on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            success: Color32::from_rgb(0x1A, 0x7F, 0x37),
            warn: Color32::from_rgb(0x9A, 0x67, 0x00),
            danger: Color32::from_rgb(0xCF, 0x22, 0x2E),
            viewport: Color32::from_rgb(0x1B, 0x20, 0x28),
            shadow: Color32::from_black_alpha(40),
        },
    }
}

/// The colour that stands for a protocol everywhere it appears.
pub fn protocol_color(protocol: Protocol) -> Color32 {
    match protocol {
        Protocol::Vnc => Color32::from_rgb(0xF9, 0x73, 0x16),
        Protocol::Rdp => Color32::from_rgb(0x3B, 0x82, 0xF6),
        Protocol::Spice => Color32::from_rgb(0xA8, 0x55, 0xF7),
    }
}

/// The family the whole interface is set in.
///
/// Noto Sans SC covers Latin as well as the Chinese ranges, so one family serves
/// both interface languages and neither looks bolted on. `assets/fonts/OFL.txt`
/// is its licence.
const UI_FONT_NAME: &str = "Noto Sans SC";
const UI_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(UI_FONT_NAME.to_owned(), Arc::new(FontData::from_static(UI_FONT)));
    // Proportional text is Noto Sans SC outright. Monospace keeps its own
    // metrics and only borrows the CJK glyphs egui's default fonts lack.
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, UI_FONT_NAME.to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push(UI_FONT_NAME.to_owned());
    fonts
}

/// Hand egui the interface font, once per context.
///
/// `set_fonts` rebuilds the glyph atlas, so it is guarded rather than repeated
/// on every theme change.
fn install_fonts(ctx: &Context) {
    let key = egui::Id::new("airframe:fonts");
    if ctx.data(|d| d.get_temp::<()>(key)).is_some() {
        return;
    }
    ctx.set_fonts(fonts());
    ctx.data_mut(|d| d.insert_temp(key, ()));
}

/// Install the palette and the shared spacing rules.
///
/// egui keeps one style per theme; AirFrame pins the preference to the one the
/// user picked and rewrites that style, so the palette is never half-applied.
pub fn apply(ctx: &Context, theme: Theme) {
    let p = palette(theme);
    let (egui_theme, preference) = match theme {
        Theme::Dark => (egui::Theme::Dark, egui::ThemePreference::Dark),
        Theme::Light => (egui::Theme::Light, egui::ThemePreference::Light),
    };
    ctx.set_theme(preference);
    install_fonts(ctx);

    ctx.style_mut_of(egui_theme, |style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();

        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(14);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.interact_size.y = 26.0;

        style.visuals = visuals(&p, theme);
    });
}

fn visuals(p: &Palette, theme: Theme) -> Visuals {
    let mut v = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };
    let radius = CornerRadius::same(8);

    v.panel_fill = p.panel;
    v.window_fill = p.elevated;
    // Windows and controls separate themselves with fill and shadow. A stroke
    // on every nested box is what makes a screen read as a wireframe.
    v.window_stroke = Stroke::NONE;
    v.window_corner_radius = CornerRadius::same(12);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: p.shadow,
    };
    v.popup_shadow = v.window_shadow;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.card;
    v.text_edit_bg_color = Some(p.bg);
    v.selection.bg_fill = p.accent.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.hyperlink_color = p.accent;
    v.menu_corner_radius = radius;
    v.code_bg_color = p.card;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.danger;

    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = p.card;
    v.widgets.inactive.weak_bg_fill = p.card;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text_dim);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = p.card_hover;
    v.widgets.hovered.weak_bg_fill = p.card_hover;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.corner_radius = radius;

    v.widgets.active.bg_fill = p.accent.gamma_multiply(0.55);
    v.widgets.active.weak_bg_fill = p.accent.gamma_multiply(0.55);
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.corner_radius = radius;

    v.widgets.open.bg_fill = p.card_hover;
    // Windows draw their title bar with the *open* fill; matching the window
    // fill keeps a dialog one flat surface instead of a bar inside a box.
    v.widgets.open.weak_bg_fill = p.elevated;
    v.widgets.open.bg_stroke = Stroke::NONE;
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.open.corner_radius = radius;

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled Noto Sans SC has to be the family the interface measures
    /// against, or Chinese renders as nothing at all.
    #[test]
    fn the_bundled_font_covers_chinese_and_latin() {
        let ctx = Context::default();
        apply(&ctx, Theme::Dark);
        // Glyphs only exist once a frame has run.
        ctx.run_ui(Default::default(), |ui| {
            ui.label("AirFrame 中文");
        })
        .drop_without_applying_deltas();

        let body = FontId::proportional(13.5);
        let (latin, han) = ctx.fonts_mut(|f| (f.glyph_width(&body, 'A'), f.glyph_width(&body, '连')));
        assert!(latin > 0.0, "Latin text must come from the bundled font, got {latin}");
        assert!(han > latin, "Han glyphs are full-width, got {han} against {latin}");
    }
}
