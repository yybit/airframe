//! The saved-connection list down the left-hand side.

use std::collections::BTreeSet;

use airframe_core::{Connection, Lang, Protocol, Theme};
use airframe_ui::theme::Palette;
use airframe_ui::{Icon, State, elide, palette};
use egui::{
    Align, Align2, CornerRadius, FontId, Frame, Layout, Margin, Rect, RichText, ScrollArea,
    Sense, TextEdit, Ui, Vec2, pos2, vec2, WidgetInfo, WidgetType,
};
use uuid::Uuid;

use crate::app::{Action, App, shortcut};

/// Height of one connection row.
const ROW: f32 = 46.0;

pub fn show(ui: &mut Ui, app: &mut App) {
    let theme = app.theme();
    let p = palette(theme);
    let mut pending: Vec<Action> = Vec::new();

    header(ui, app, &p, &mut pending);
    search(ui, app, &p);

    let list_height = (ui.available_height() - 54.0).max(90.0);
    ScrollArea::vertical()
        .max_height(list_height)
        .auto_shrink([false, false])
        .show(ui, |ui| body(ui, app, theme, &p, &mut pending));

    app.actions.append(&mut pending);
}

fn header(ui: &mut Ui, app: &App, p: &Palette, pending: &mut Vec<Action>) {
    let lang = app.lang();
    ui.horizontal(|ui| {
        ui.label(RichText::new(lang.pick("Connections", "连接")).strong().size(13.0).color(p.text));
        ui.label(
            RichText::new(app.store.data.connections.len().to_string())
                .size(11.0)
                .color(p.text_faint),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let hint = format!("{} ({})", lang.pick("New connection", "新建连接"), shortcut("T"));
            if airframe_ui::icon_button(ui, Icon::Plus, 24.0, p.text_dim, &hint).clicked() {
                pending.push(Action::New);
            }
        });
    });
    ui.add_space(6.0);
}

fn search(ui: &mut Ui, app: &mut App, p: &Palette) {
    let lang = app.lang();
    Frame {
        fill: p.bg,
        corner_radius: CornerRadius::same(9),
        inner_margin: Margin::symmetric(9, 4),
        ..Frame::NONE
    }
    .show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
            airframe_ui::icons::paint(ui.painter(), rect, Icon::Search, p.text_faint);
            ui.add(
                TextEdit::singleline(&mut app.search)
                    .frame(Frame::NONE)
                    .hint_text(lang.pick("Search", "搜索"))
                    .vertical_align(Align::Center),
            );
        });
    });
    ui.add_space(8.0);
}

/// A connection, flattened for display.
///
/// The list is rebuilt every frame from owned rows so drawing never holds a
/// borrow of the store while it queues actions.
struct Row {
    id: Uuid,
    protocol: Protocol,
    name: String,
    subtitle: String,
    state: Option<State>,
}

fn body(ui: &mut Ui, app: &mut App, theme: Theme, p: &Palette, pending: &mut Vec<Action>) {
    let lang = app.lang();
    let groups = plan(app);
    if groups.is_empty() {
        ui.add_space(20.0);
        let message = if app.store.data.connections.is_empty() {
            lang.pick("No connections yet", "还没有任何连接")
        } else {
            lang.pick("Nothing matches that search", "没有匹配的搜索结果")
        };
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(message).size(12.0).color(p.text_faint));
        });
        return;
    }

    for (group, rows) in groups {
        let open = !app.collapsed_groups.contains(&group);
        group_header(ui, p, &group, rows.len(), open, &mut app.collapsed_groups);
        if !open {
            continue;
        }
        for row in &rows {
            let selected = app.selection == Some(row.id);
            row_widget(ui, theme, lang, p, row, selected, pending);
        }
    }
}

fn plan(app: &App) -> Vec<(String, Vec<Row>)> {
    let lang = app.lang();
    let needle = app.search.trim().to_lowercase();
    let mut groups: Vec<(String, Vec<Row>)> = Vec::new();

    for connection in &app.store.data.connections {
        if !needle.is_empty() {
            let haystack = format!(
                "{} {} {} {}",
                connection.name,
                connection.host,
                connection.protocol.label(),
                connection.group
            )
            .to_lowercase();
            if !haystack.contains(&needle) {
                continue;
            }
        }

        let state = app
            .tabs
            .iter()
            .find(|tab| tab.session.connection.id == connection.id)
            .map(|tab| tab.session.state);

        let row = Row {
            id: connection.id,
            protocol: connection.protocol,
            name: if connection.name.trim().is_empty() {
                connection.host.clone()
            } else {
                connection.name.clone()
            },
            subtitle: subtitle(connection),
            state,
        };

        let group = match connection.group.trim() {
            "" => lang.pick("Ungrouped", "未分组").to_owned(),
            named => named.to_owned(),
        };
        match groups.iter_mut().find(|(name, _)| *name == group) {
            Some((_, rows)) => rows.push(row),
            None => groups.push((group, vec![row])),
        }
    }

    groups
}

fn subtitle(connection: &Connection) -> String {
    match connection.username.trim() {
        "" => connection.address(),
        user => format!("{user}@{}", connection.address()),
    }
}

fn group_header(
    ui: &mut Ui,
    p: &Palette,
    group: &str,
    count: usize,
    open: bool,
    collapsed: &mut BTreeSet<String>,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
        let icon_rect =
            Rect::from_center_size(pos2(rect.left() + 10.0, rect.center().y), Vec2::splat(14.0));
        airframe_ui::icons::paint(ui.painter(), icon_rect, chevron, p.text_faint);
        ui.painter().text(
            pos2(rect.left() + 22.0, rect.center().y),
            Align2::LEFT_CENTER,
            group.to_uppercase(),
            FontId::proportional(10.0),
            p.text_faint,
        );
        ui.painter().text(
            pos2(rect.right() - 2.0, rect.center().y),
            Align2::RIGHT_CENTER,
            count.to_string(),
            FontId::proportional(10.0),
            p.text_faint,
        );
    }
    if response.clicked() {
        if open {
            collapsed.insert(group.to_owned());
        } else {
            collapsed.remove(group);
        }
    }
}

fn row_widget(
    ui: &mut Ui,
    theme: Theme,
    lang: Lang,
    p: &Palette,
    row: &Row,
    selected: bool,
    pending: &mut Vec<Action>,
) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Button, true, selected, row.name.clone())
    });
    if !ui.is_rect_visible(rect) {
        return;
    }

    let hovered = response.hovered();
    if selected || hovered {
        let fill = if selected { p.card } else { p.card_hover };
        ui.painter()
            .rect_filled(rect.shrink2(vec2(2.0, 1.0)), CornerRadius::same(8), fill);
    }
    if let Some(state) = row.state {
        let bar = Rect::from_min_size(
            pos2(rect.left() + 2.0, rect.top() + 9.0),
            vec2(3.0, ROW - 19.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), state.color(theme));
    }
    ui.painter().circle_filled(
        pos2(rect.left() + 17.0, rect.center().y),
        4.5,
        airframe_ui::protocol_color(row.protocol),
    );

    let edit_rect =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), Vec2::splat(22.0));
    let delete_rect =
        Rect::from_center_size(pos2(rect.right() - 42.0, rect.center().y), Vec2::splat(22.0));
    let pointer = ui.ctx().pointer_latest_pos();
    let show_actions = hovered || selected;
    let over = |target: Rect| hovered && pointer.is_some_and(|at| target.contains(at));
    let (over_edit, over_delete) = (over(edit_rect), over(delete_rect));

    let text_left = rect.left() + 31.0;
    let text_right = if show_actions { delete_rect.left() - 6.0 } else { rect.right() - 10.0 };
    let room = (text_right - text_left).max(24.0);
    ui.painter().text(
        pos2(text_left, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        elide(&row.name, room, 13.0),
        FontId::proportional(13.0),
        p.text,
    );
    ui.painter().text(
        pos2(text_left, rect.center().y + 8.5),
        Align2::LEFT_CENTER,
        elide(&row.subtitle, room, 11.0),
        FontId::proportional(11.0),
        p.text_faint,
    );

    if show_actions {
        paint_action(ui, edit_rect, Icon::Pencil, over_edit, p);
        paint_action(ui, delete_rect, Icon::Trash, over_delete, p);
    }

    if response.clicked() {
        if over_delete {
            pending.push(Action::Delete(row.id));
        } else if over_edit {
            pending.push(Action::Edit(row.id));
        } else {
            pending.push(Action::Connect(row.id));
        }
    }

    response.context_menu(|ui| {
        if ui.button(lang.pick("Connect", "连接")).clicked() {
            pending.push(Action::Connect(row.id));
            ui.close();
        }
        if ui.button(lang.pick("Edit\u{2026}", "编辑\u{2026}")).clicked() {
            pending.push(Action::Edit(row.id));
            ui.close();
        }
        if ui.button(lang.pick("Duplicate", "复制")).clicked() {
            pending.push(Action::Duplicate(row.id));
            ui.close();
        }
        ui.separator();
        if ui.button(lang.pick("Delete", "删除")).clicked() {
            pending.push(Action::Delete(row.id));
            ui.close();
        }
    });
}

/// Paint one of a row's trailing actions.
///
/// They are decided from the row's own click, by pointer position, rather than
/// being widgets of their own: two hit areas that overlap resolve in an order
/// that let the row swallow the click, so the icons looked dead.
fn paint_action(ui: &Ui, rect: Rect, icon: Icon, hot: bool, p: &Palette) {
    if hot {
        ui.painter().rect_filled(rect, CornerRadius::same(6), p.elevated);
    }
    airframe_ui::icons::paint(ui.painter(), rect, icon, if hot { p.text } else { p.text_dim });
}
