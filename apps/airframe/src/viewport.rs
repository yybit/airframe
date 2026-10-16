//! The remote framebuffer: its overlay toolbar, remote cursor and all input.

use std::f32::consts::TAU;

use airframe_core::{Lang, Protocol, SessionCommand, Theme};
use airframe_ui::{Icon, State, palette};
use egui::{
    Align, Align2, Area, Color32, CornerRadius, Frame, FontId, Id, Layout, Margin, Order, Painter,
    Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2,
};

use crate::app::{Action, Tab};
use crate::keymap;
use crate::session::{Session, Zoom};

/// How much wheel travel makes one notch.
const NOTCH: f32 = 42.0;

/// Show the active session, returning anything the user asked for.
pub fn show(
    ui: &mut Ui,
    tab: &mut Tab,
    index: usize,
    theme: Theme,
    lang: Lang,
    dialogs_open: bool,
) -> Vec<Action> {
    let p = palette(theme);
    let mut actions = Vec::new();

    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, CornerRadius::same(10), p.viewport);

    toolbar(ui, rect, index, tab, theme, lang, &mut actions);

    let session = &mut tab.session;

    if session.size.0 == 0 || session.size.1 == 0 {
        status_overlay(&painter, rect, session, theme, lang, false);
        return actions;
    }

    let (w, h) = session.size;
    let image = match session.zoom {
        Zoom::Fit => {
            let scale = (rect.width() / w as f32).min(rect.height() / h as f32);
            Rect::from_center_size(rect.center(), vec2(w as f32 * scale, h as f32 * scale))
        }
        Zoom::Actual => Rect::from_center_size(rect.center(), vec2(w as f32, h as f32)),
        Zoom::Stretch => rect,
    };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

    if let Some(texture) = session.texture() {
        painter.image(texture.id(), image, uv, Color32::WHITE);
    }

    let scale_x = image.width() / w as f32;
    let scale_y = image.height() / h as f32;
    let mut remote_cursor = false;
    if session.cursor_visible
        && let (Some(texture), Some(cursor)) =
            (session.cursor_texture(), session.cursor_image())
    {
        remote_cursor = true;
        let origin = image.min
            + vec2(
                (session.cursor_pos.0 - cursor.hotspot_x as i32) as f32 * scale_x,
                (session.cursor_pos.1 - cursor.hotspot_y as i32) as f32 * scale_y,
            );
        let size = vec2(cursor.width as f32 * scale_x, cursor.height as f32 * scale_y);
        if size.x >= 1.0 && size.y >= 1.0 {
            painter.image(texture.id(), Rect::from_min_size(origin, size), uv, Color32::WHITE);
        }
    }

    if response.hovered() && remote_cursor {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }

    if session.state != State::Live {
        status_overlay(&painter, rect, session, theme, lang, true);
        return actions;
    }

    // A live view is worth reporting on.
    painter.text(
        pos2(rect.left() + 12.0, rect.bottom() - 9.0),
        Align2::LEFT_BOTTOM,
        format!("{:.0} fps   {}", session.fps, human_bytes(session.received_bytes)),
        FontId::proportional(10.5),
        p.text_faint,
    );

    if dialogs_open {
        return actions;
    }

    pointer_input(ui, &response, session, image, (w, h));
    scroll_input(ui, &response, session);
    key_input(ui, session);

    actions
}

/// Everything the placeholder screen (no session yet) needs.
pub fn onboarding(ui: &mut Ui, theme: Theme, lang: Lang) -> Vec<Action> {
    let p = palette(theme);
    let mut actions = Vec::new();

    ui.add_space((ui.available_height() * 0.16).min(140.0));
    ui.vertical_centered(|ui| {
        let (mark, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
        airframe_ui::logo::paint(ui, mark);
        ui.add_space(10.0);
        ui.label(RichText::new("AirFrame").size(30.0).strong().color(p.text));
        ui.add_space(6.0);
        ui.label(
            RichText::new(lang.pick(
                "VNC, RDP and SPICE desktops in one native window.",
                "在一个原生窗口中连接 VNC、RDP 和 SPICE 桌面。",
            ))
            .size(13.0)
            .color(p.text_dim),
        );

        ui.add_space(22.0);
        // The row is measured before it is placed: a bare `horizontal` claims the
        // whole width, and then `vertical_centered` has nothing left to centre.
        const BADGE_GAP: f32 = 6.0;
        let badges: Vec<Vec2> = Protocol::ALL
            .iter()
            .map(|protocol| airframe_ui::badge_size(ui, protocol.label()))
            .collect();
        let row = vec2(
            badges.iter().map(|badge| badge.x).sum::<f32>()
                + BADGE_GAP * (badges.len() - 1) as f32,
            badges.iter().map(|badge| badge.y).fold(0.0, f32::max),
        );
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            // The gap is ours, so the measurement above is the row's exact width.
            ui.spacing_mut().item_spacing.x = 0.0;
            for (index, protocol) in Protocol::ALL.iter().enumerate() {
                if index > 0 {
                    ui.add_space(BADGE_GAP);
                }
                airframe_ui::solid_badge(
                    ui,
                    protocol.label(),
                    airframe_ui::protocol_color(*protocol),
                    Color32::WHITE,
                );
            }
        });

        ui.add_space(26.0);
        let label =
            RichText::new(lang.pick("New connection", "新建连接")).color(p.on_accent).size(13.0);
        let new = egui::Button::new(label)
            .fill(p.accent)
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(196.0, 34.0));
        if ui.add(new).clicked() {
            actions.push(Action::New);
        }

        ui.add_space(30.0);
        let hint = lang.pick(
            "Ctrl+T new   \u{b7}   Ctrl+W close   \u{b7}   Ctrl+Tab switch",
            "Ctrl+T 新建   \u{b7}   Ctrl+W 关闭会话   \u{b7}   Ctrl+Tab 切换",
        );
        ui.label(RichText::new(hint).size(11.0).color(p.text_faint));
    });

    actions
}

fn pointer_input(
    ui: &Ui,
    response: &egui::Response,
    session: &mut Session,
    image: Rect,
    size: (u32, u32),
) {
    let (w, h) = size;
    let pointer = ui.input(|i| i.pointer.clone());
    let buttons = Session::buttons_from_egui(&pointer);
    let Some(pos) = ui.ctx().pointer_latest_pos() else {
        session.down_buttons = 0;
        return;
    };

    let fx = ((pos.x - image.min.x) / image.width()).clamp(0.0, 1.0);
    let fy = ((pos.y - image.min.y) / image.height()).clamp(0.0, 1.0);
    let x = ((fx * w as f32) as u32).min(w - 1) as u16;
    let y = ((fy * h as f32) as u32).min(h - 1) as u16;

    let previous = session.down_buttons;
    let moved = session.pointer != Some((x, y));
    session.pointer = Some((x, y));
    session.down_buttons = buttons;

    if session.view_only {
        return;
    }
    if !response.hovered() && buttons == 0 {
        return;
    }
    if moved || buttons != previous {
        session.send(SessionCommand::Pointer { x, y, buttons, moved });
    }
}

fn scroll_input(ui: &Ui, response: &egui::Response, session: &mut Session) {
    if session.view_only || !response.hovered() {
        return;
    }
    // Take the wheel for the remote desktop rather than letting a panel under
    // the pointer scroll.
    let delta = ui.input_mut(|input| {
        let value = input.smooth_scroll_delta;
        input.smooth_scroll_delta = Vec2::ZERO;
        value
    });
    if delta == Vec2::ZERO {
        return;
    }

    let (ax, ay) = session.scroll_accum;
    let (ax, ay) = (ax + delta.x, ay + delta.y);
    let (nx, ny) = ((ax / NOTCH).trunc(), (ay / NOTCH).trunc());
    session.scroll_accum = (ax - nx * NOTCH, ay - ny * NOTCH);
    if nx == 0.0 && ny == 0.0 {
        return;
    }

    let (x, y) = session.pointer.unwrap_or((0, 0));
    session.send(SessionCommand::Scroll {
        x,
        y,
        dx: nx as i16,
        dy: ny as i16,
    });
}

fn key_input(ui: &Ui, session: &mut Session) {
    if session.view_only || ui.ctx().egui_wants_keyboard_input() {
        return;
    }

    // Modifier keys never arrive as key events, so their transitions are
    // derived from the modifier state and synthesised here.
    let held = ui.input(|i| keymap::modifier_state(&i.modifiers));
    for (index, down) in held.into_iter().enumerate() {
        if down != session.modifiers[index] {
            session.modifiers[index] = down;
            session.send(SessionCommand::Key(keymap::MODIFIERS[index].input(down)));
        }
    }

    for event in ui.input(|i| i.events.clone()) {
        match event {
            egui::Event::Key { key, physical_key, pressed, .. } => {
                if let Some(code) = keymap::lookup(physical_key.unwrap_or(key)) {
                    session.send(SessionCommand::Key(code.input(pressed)));
                }
            }
            egui::Event::Paste(text) => session.send(SessionCommand::ClipboardText(text)),
            _ => {}
        }
    }
}

/// How each zoom mode is presented in the toolbar.
///
/// The mark and the wording live here, next to the buttons that show them,
/// rather than in the session state they act on.
impl Zoom {
    fn icon(self) -> Icon {
        match self {
            Zoom::Fit => Icon::Fit,
            Zoom::Actual => Icon::Pixel,
            Zoom::Stretch => Icon::Stretch,
        }
    }

    fn hint(self, lang: Lang) -> &'static str {
        match self {
            Zoom::Fit => lang.pick("Fit the desktop to the window", "适应窗口"),
            Zoom::Actual => lang.pick("One image pixel per screen pixel", "原始大小"),
            Zoom::Stretch => lang.pick("Stretch the desktop to fill the window", "拉伸填满窗口"),
        }
    }
}

/// The floating control strip in the viewport's top-right corner.
fn toolbar(
    ui: &Ui,
    rect: Rect,
    index: usize,
    tab: &mut Tab,
    theme: Theme,
    lang: Lang,
    actions: &mut Vec<Action>,
) {
    let tab_id = tab.id;
    let p = palette(theme);
    let ctx = ui.ctx().clone();
    let session = &mut tab.session;
    let resizable = session.connection.protocol == Protocol::Rdp;

    Area::new(Id::new(("airframe-viewport-toolbar", tab_id)))
        .order(Order::Foreground)
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(rect.right_top() + vec2(-12.0, 12.0))
        .constrain(true)
        .show(&ctx, |ui| {
            Frame {
                fill: p.elevated,
                corner_radius: CornerRadius::same(10),
                inner_margin: Margin::symmetric(7, 5),
                shadow: egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 20,
                    spread: 0,
                    color: Color32::from_black_alpha(70),
                },
                ..Frame::NONE
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for zoom in Zoom::ALL {
                        let response = airframe_ui::icon_toggle(
                            ui,
                            zoom.icon(),
                            24.0,
                            p.text_dim,
                            session.zoom == zoom,
                            zoom.hint(lang),
                        );
                        if response.clicked() {
                            session.zoom = zoom;
                        }
                    }
                    ui.add_space(4.0);
                    if airframe_ui::icon_toggle(
                        ui,
                        Icon::Eye,
                        24.0,
                        p.text_dim,
                        session.view_only,
                        lang.pick(
                            "View only: watch without sending input",
                            "只读查看：仅观看，不发送输入",
                        ),
                    )
                    .clicked()
                    {
                        session.view_only = !session.view_only;
                    }
                    if resizable
                        && airframe_ui::icon_button(
                            ui,
                            Icon::Expand,
                            24.0,
                            p.text_dim,
                            lang.pick(
                                "Resize the remote desktop to this window",
                                "将远程桌面调整为当前窗口大小",
                            ),
                        )
                        .clicked()
                    {
                        let width = rect.width().round().clamp(200.0, 8192.0) as u16;
                        let height = rect.height().round().clamp(200.0, 8192.0) as u16;
                        session.request_resize(width, height);
                    }
                    ui.add_space(4.0);
                    if airframe_ui::icon_button(
                        ui,
                        Icon::Camera,
                        24.0,
                        p.text_dim,
                        lang.pick(
                            "Save a screenshot of the desktop",
                            "保存当前桌面截图",
                        ),
                    )
                    .clicked()
                    {
                        actions.push(Action::Screenshot(index));
                    }
                    if airframe_ui::icon_button(
                        ui,
                        Icon::Keyboard,
                        24.0,
                        p.text_dim,
                        lang.pick(
                            "Send text to the remote (Ctrl+Shift+V): typed in one key at a time, for consoles with no clipboard",
                            "向远端发送文本（Ctrl+Shift+V）：逐键输入，适用于没有剪贴板的控制台",
                        ),
                    )
                    .clicked()
                    {
                        actions.push(Action::SendText);
                    }
                    ui.add_space(4.0);
                    let hint = lang.pick("Reconnect", "重新连接");
                    if airframe_ui::icon_button(ui, Icon::Refresh, 24.0, p.text_dim, hint)
                        .clicked()
                    {
                        actions.push(Action::Reconnect(index));
                    }
                    let hint = lang.pick("Disconnect", "断开连接");
                    if airframe_ui::icon_button(ui, Icon::Power, 24.0, p.danger, hint)
                        .clicked()
                    {
                        session.disconnect();
                    }
                    let hint = lang.pick("Close session", "关闭会话");
                    if airframe_ui::icon_button(ui, Icon::Close, 24.0, p.text_dim, hint)
                        .clicked()
                    {
                        actions.push(Action::Close(index));
                    }
                });
            });
        });
}

/// The centred message shown while there is no live frame to look at.
fn status_overlay(
    painter: &Painter,
    rect: Rect,
    session: &Session,
    theme: Theme,
    lang: Lang,
    scrim: bool,
) {
    let p = palette(theme);
    let center = rect.center();

    if scrim {
        painter.rect_filled(rect, CornerRadius::same(10), Color32::from_black_alpha(160));
    }

    match session.state {
        State::Connecting => {
            let time = painter.ctx().input(|i| i.time) as f32;
            spinner(painter, center - vec2(0.0, 44.0), time, p.accent);
            painter.text(
                center - vec2(0.0, 10.0),
                Align2::CENTER_CENTER,
                &session.name,
                FontId::proportional(16.0),
                p.text,
            );
            painter.text(
                center + vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                &session.status,
                FontId::proportional(12.0),
                p.text_dim,
            );
        }
        State::Failed => {
            painter.text(
                center - vec2(0.0, 36.0),
                Align2::CENTER_CENTER,
                &session.name,
                FontId::proportional(16.0),
                p.text,
            );
            let message = session.error.clone().unwrap_or_else(|| session.status.clone());
            painter.text(
                center - vec2(0.0, 10.0),
                Align2::CENTER_CENTER,
                message,
                FontId::proportional(12.0),
                p.danger,
            );
            painter.text(
                center + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                lang.pick(
                    "Recover with the Reconnect button in the toolbar.",
                    "请使用工具栏中的“重新连接”按钮恢复。",
                ),
                FontId::proportional(11.0),
                p.text_faint,
            );
        }
        _ => {
            painter.text(
                center - vec2(0.0, 8.0),
                Align2::CENTER_CENTER,
                lang.pick("Disconnected", "已断开"),
                FontId::proportional(15.0),
                p.text_dim,
            );
            painter.text(
                center + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                lang.pick(
                    "Reconnect from the toolbar to pick the session back up.",
                    "请从工具栏重新连接以继续此会话。",
                ),
                FontId::proportional(11.0),
                p.text_faint,
            );
        }
    }
}

fn spinner(painter: &Painter, center: Pos2, time: f32, color: Color32) {
    const STEPS: usize = 30;
    let start = time * 3.4;
    let points: Vec<Pos2> = (0..=STEPS)
        .map(|i| {
            let angle = start + (i as f32 / STEPS as f32) * TAU * 0.72;
            pos2(center.x + 11.0 * angle.cos(), center.y + 11.0 * angle.sin())
        })
        .collect();
    painter.add(Shape::line(points, Stroke::new(2.2, color)));
}

fn human_bytes(count: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let value = count as f64;
    if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.0} KiB", value / KIB)
    } else {
        format!("{count} B")
    }
}
