//! The application shell: saved connections, open tabs and the chrome around them.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use airframe_core::{Connection, Lang, Store, StoreError, Theme};
use airframe_ui::{Icon, State, palette, theme as ui_theme};
use egui::{
    Align, Align2, Area, CentralPanel, Color32, Context, CornerRadius, Frame, Id, Key,
    KeyboardShortcut, Layout, Margin, Modifiers, Order, Panel, RichText, Sense, Stroke, Ui, Vec2,
    ViewportCommand, vec2,
};
use uuid::Uuid;

use crate::editor::{Editor, Outcome};
use crate::session::Session;
use crate::{editor, sidebar, viewport};

/// How often a live session is asked to repaint.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// How long a toast stays on screen.
const TOAST_LIFETIME: Duration = Duration::from_millis(2800);

/// Something the chrome asked the app to do.
///
/// Widgets push these instead of mutating shared state directly, so a frame's
/// worth of clicks is applied in one place, after the UI has been built.
#[derive(Clone)]
pub enum Action {
    Connect(Uuid),
    New,
    Edit(Uuid),
    Duplicate(Uuid),
    Delete(Uuid),
    Close(usize),
    Reconnect(usize),
    Screenshot(usize),
    /// Open the dialog that types text into the remote session.
    SendText,
    Settings,
    Fullscreen,
    Quit,
}

/// One open session.
pub struct Tab {
    /// Stable for the tab's lifetime; used for egui widget ids.
    pub id: u64,
    pub session: Session,
}

#[derive(Clone, Copy)]
enum Notice {
    Info,
    Success,
    Error,
}

#[derive(Clone)]
struct Toast {
    text: String,
    notice: Notice,
    until: Instant,
}

/// A password prompt for a session whose server asked for a password the
/// connection does not have.
#[derive(Clone)]
pub struct PasswordPrompt {
    /// Index into `App::tabs` of the session that needs a password.
    pub tab: usize,
    pub buffer: String,
    pub remember: bool,
}

pub struct App {
    pub store: Store,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub selection: Option<Uuid>,
    pub search: String,
    pub editor: Option<Editor>,
    pub settings_open: bool,
    pub confirm_delete: Option<Uuid>,
    /// Open when the "send text to the remote" dialog is on screen.
    pub send_text: Option<String>,
    /// Open when a session's server asks for a password the connection lacks.
    pub password_prompt: Option<PasswordPrompt>,
    /// Passwords typed without "remember on this device": kept for this run
    /// only, never written to the store. Keyed by connection id.
    run_passwords: HashMap<Uuid, String>,
    pub collapsed_groups: BTreeSet<String>,
    pub actions: Vec<Action>,
    toast: Option<Toast>,
    next_tab: u64,
    applied_theme: Option<Theme>,
}

impl App {
    pub fn new(store: Store, startup_error: Option<StoreError>) -> Self {
        let mut app = Self {
            store,
            tabs: Vec::new(),
            active: 0,
            selection: None,
            search: String::new(),
            editor: None,
            settings_open: false,
            confirm_delete: None,
            send_text: None,
            password_prompt: None,
            run_passwords: HashMap::new(),
            collapsed_groups: BTreeSet::new(),
            actions: Vec::new(),
            toast: None,
            next_tab: 0,
            // Applied on the first frame, once we have the live context.
            applied_theme: None,
        };
        if let Some(error) = startup_error {
            app.notify(error.to_string(), Notice::Error);
        }
        app
    }

    pub fn theme(&self) -> Theme {
        self.store.data.settings.theme
    }

    pub fn lang(&self) -> Lang {
        self.store.data.settings.language
    }

    fn notify(&mut self, text: String, notice: Notice) {
        self.toast =
            Some(Toast { text, notice, until: Instant::now() + TOAST_LIFETIME });
    }

    /// Write the session's current picture to the user's pictures folder.
    fn save_screenshot(&mut self, index: usize) {
        let lang = self.lang();
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let Some(png) = tab.session.screenshot() else {
            let text = lang.pick("There is no picture to save yet", "目前还没有可保存的画面");
            self.notify(text.into(), Notice::Error);
            return;
        };
        let directory = screenshot_dir(self.store.path());
        let path = unique_path(&directory, &screenshot_stem(&tab.session.name));
        let result = std::fs::create_dir_all(&directory).and_then(|()| std::fs::write(&path, png));
        match result {
            Ok(()) => {
                let text = format!(
                    "{} {}",
                    lang.pick("Screenshot saved to", "截图已保存到"),
                    path.display()
                );
                self.notify(text, Notice::Success);
            }
            Err(error) => {
                let text =
                    format!("{}: {error}", lang.pick("Could not save the screenshot", "无法保存截图"));
                self.notify(text, Notice::Error);
            }
        }
    }

    fn save(&mut self) {
        if let Err(error) = self.store.save() {
            let lang = self.lang();
            let text = format!("{}: {error}", lang.pick("Could not save settings", "无法保存设置"));
            self.notify(text, Notice::Error);
        }
    }

    /// Open a session for `connection`, reusing its tab when one already exists.
    fn open(&mut self, mut connection: Connection) {
        let conn_id = connection.id;
        // A password typed without "remember on this device" lives for this
        // run only; reattach it so connecting from the sidebar works too.
        if connection.password.is_none()
            && let Some(password) = self.run_passwords.get(&conn_id)
        {
            connection.password = Some(password.clone());
        }
        if let Some(index) = self.tabs.iter().position(|tab| {
            tab.session.connection.id == conn_id
        }) {
            let tab = &mut self.tabs[index];
            // Refresh the session's config (e.g. a newly typed password). A
            // failed session restarts so saving a corrected password reconnects.
            let failed = tab.session.state == State::Failed;
            tab.session.connection = connection;
            if failed {
                tab.session.restart();
            }
            self.active = index;
            self.selection = Some(conn_id);
            return;
        }
        if self.store.data.settings.view_only_default {
            connection.view_only = true;
        }
        self.selection = Some(conn_id);
        let id = self.next_tab;
        self.next_tab += 1;
        let lang = self.lang();
        self.tabs.push(Tab { id, session: Session::new(connection, lang) });
        self.active = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        // The sidebar is what shows which connection the workspace is on, so it
        // moves to whichever session took the closed one's place.
        if let Some(tab) = self.tabs.get(self.active) {
            self.selection = Some(tab.session.connection.id);
        }
    }

    fn commit(&mut self, connection: Connection, connect: bool) {
        let lang = self.lang();
        if connect {
            self.open(connection.clone());
        }
        let name = connection.display_name().to_owned();
        self.selection = Some(connection.id);
        // The store scrubs un-remembered passwords, so park one for this run
        // when the user typed it without "remember on this device".
        match connection.password.as_deref() {
            Some(password) if !connection.remember_password => {
                self.run_passwords.insert(connection.id, password.to_owned());
            }
            _ => {
                self.run_passwords.remove(&connection.id);
            }
        }
        self.store.upsert(connection);
        self.save();
        let text = format!("{} \u{201c}{name}\u{201d}", lang.pick("Saved", "已保存"));
        self.notify(text, Notice::Success);
    }

    /// Drain every backend queue and keep the frame loop running while they work.
    fn pump(&mut self, ctx: &Context) {
        let lang = self.lang();
        let mut busy = false;
        let mut copied = false;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            // Text the remote copied comes back through the protocol; the local
            // clipboard and the word about it are the window's business.
            if let Some(text) = tab.session.pump(ctx, lang) {
                ctx.copy_text(text);
                copied = true;
            }
            if matches!(tab.session.state, State::Live | State::Connecting) {
                busy = true;
            }
            if tab.session.needs_password && self.password_prompt.is_none() {
                self.password_prompt = Some(PasswordPrompt {
                    tab: index,
                    buffer: String::new(),
                    remember: tab.session.connection.remember_password,
                });
            }
        }
        if copied {
            let text = lang.pick(
                "Copied to this computer's clipboard",
                "已复制到本机剪贴板",
            );
            self.notify(text.into(), Notice::Info);
        }
        if busy {
            ctx.request_repaint_after(FRAME_INTERVAL);
        }
    }

    fn shortcuts(&mut self, ctx: &Context) {
        if self.editor.is_some()
            || self.settings_open
            || self.confirm_delete.is_some()
            || self.send_text.is_some()
            || self.password_prompt.is_some()
        {
            return;
        }
        // A live view holds the keyboard focus so the remote gets Tab and the
        // arrows, so only a text field counts as "the user is typing".
        let typing = ctx.text_edit_focused();
        let shift_ctrl = Modifiers { shift: true, ..Modifiers::CTRL };

        let mut new = false;
        let mut close = false;
        let mut next = false;
        let mut previous = false;
        let mut settings = false;
        let mut fullscreen = false;
        let mut quit = false;
        let mut send_text = false;
        ctx.input_mut(|input| {
            if !typing {
                new = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::T));
                close = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::W));
                send_text = input.consume_shortcut(&KeyboardShortcut::new(shift_ctrl, Key::V));
                next = input.consume_shortcut(&KeyboardShortcut::new(shift_ctrl, Key::Tab));
                // Matching is exact, so the plain chord is still free after
                // the shifted one has been taken.
                previous =
                    input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Tab));
                settings =
                    input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Comma));
                quit = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Q));
            }
            fullscreen = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, Key::F11));
        });

        if quit {
            self.actions.push(Action::Quit);
        }
        if new {
            self.actions.push(Action::New);
        }
        if close && !self.tabs.is_empty() {
            self.actions.push(Action::Close(self.active));
        }
        if send_text && !self.tabs.is_empty() {
            self.actions.push(Action::SendText);
        }
        if settings {
            self.settings_open = true;
        }
        if fullscreen {
            self.actions.push(Action::Fullscreen);
        }
        if !self.tabs.is_empty() {
            if next {
                self.active = (self.active + 1) % self.tabs.len();
            }
            if previous {
                self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
            }
            // The sidebar is what shows the sessions, so its highlight follows
            // the one the workspace is on.
            if (next || previous) && let Some(tab) = self.tabs.get(self.active) {
                self.selection = Some(tab.session.connection.id);
            }
        }
    }

    fn apply_actions(&mut self, ctx: &Context) {
        let actions = std::mem::take(&mut self.actions);
        if actions.is_empty() {
            return;
        }
        let lang = self.lang();
        for action in actions {
            match action {
                Action::Connect(id) => {
                    if let Some(connection) = self.store.get(id).cloned() {
                        self.open(connection);
                    }
                }
                Action::New => self.editor = Some(Editor::new()),
                Action::Edit(id) => {
                    if let Some(connection) = self.store.get(id).cloned() {
                        self.editor = Some(Editor::edit(&connection));
                    }
                }
                Action::Duplicate(id) => {
                    if let Some(connection) = self.store.get(id).cloned() {
                        self.editor = Some(Editor::duplicate(&connection, lang));
                    }
                }
                Action::Delete(id) => self.confirm_delete = Some(id),
                Action::Close(index) => self.close_tab(index),
                Action::Reconnect(index) => {
                    if let Some(tab) = self.tabs.get_mut(index) {
                        tab.session.restart();
                    }
                }
                Action::Screenshot(index) => self.save_screenshot(index),
                Action::SendText => self.send_text = Some(String::new()),
                Action::Settings => self.settings_open = true,
                Action::Fullscreen => {
                    let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!full));
                }
                Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
            }
        }
        ctx.request_repaint();
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let theme = self.theme();
        let lang = self.lang();
        let p = palette(theme);
        ui.horizontal(|ui| {
            let (mark, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            airframe_ui::logo::paint(ui, mark);
            ui.add_space(4.0);
            ui.label(RichText::new("AirFrame").strong().size(15.0).color(p.text));
            ui.add_space(2.0);
            ui.label(RichText::new("VNC \u{b7} RDP \u{b7} SPICE").size(11.0).color(p.text_faint));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hint = lang.pick("Settings (Ctrl+,)", "设置 (Ctrl+,)");
                if airframe_ui::icon_button(ui, Icon::Gear, 26.0, p.text_dim, hint).clicked() {
                    self.actions.push(Action::Settings);
                }
                let hint = lang.pick("Fullscreen (F11)", "全屏 (F11)");
                if airframe_ui::icon_button(ui, Icon::Expand, 26.0, p.text_dim, hint).clicked() {
                    self.actions.push(Action::Fullscreen);
                }
            });
        });
    }

    fn workspace(&mut self, ui: &mut Ui, theme: Theme, lang: Lang) {
        if self.tabs.is_empty() {
            let actions = viewport::onboarding(ui, theme, lang);
            self.actions.extend(actions);
            return;
        }
        // The sidebar lists every connection and marks the ones with a live
        // session, so it doubles as the session switcher; the workspace shows
        // the one it has selected, and there is no tab strip on top of it.
        let active = self.active;
        let dialogs_open = self.editor.is_some()
            || self.settings_open
            || self.confirm_delete.is_some()
            || self.send_text.is_some()
            || self.password_prompt.is_some();
        let actions = match self.tabs.get_mut(active) {
            Some(tab) => viewport::show(ui, tab, active, theme, lang, dialogs_open),
            None => Vec::new(),
        };
        self.actions.extend(actions);
    }

    fn dialogs(&mut self, ctx: &Context) {
        let theme = self.theme();
        let lang = self.lang();

        let mut request = None;
        if let Some(editor) = &mut self.editor
            && let Some(outcome) = editor::show(ctx, editor, theme, lang)
        {
            request = Some((outcome, editor.connection.clone()));
        }
        if let Some((outcome, connection)) = request {
            self.editor = None;
            if let Outcome::Save { connect } = outcome {
                self.commit(connection, connect);
            }
        }

        self.settings_window(ctx, theme);
        self.delete_dialog(ctx, theme);
        self.send_text_dialog(ctx, theme);
        self.password_prompt_dialog(ctx, theme);
    }

    fn settings_window(&mut self, ctx: &Context, theme: Theme) {
        if !self.settings_open {
            return;
        }
        let p = palette(theme);
        let mut open = true;
        let mut changed = false;
        // Import/export need a native file dialog, which Android has none of.
        #[cfg(not(target_os = "android"))]
        let mut import = false;
        #[cfg(not(target_os = "android"))]
        let mut export = false;
        let lang = self.lang();

        egui::Window::new(lang.pick("Settings", "设置"))
            .id(Id::new("airframe-settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(440.0)
            .frame(dialog_frame(theme))
            .show(ctx, |ui| {
                ui.label(RichText::new(lang.pick("Appearance", "外观")).strong().color(p.text));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lang.pick("Theme", "主题")).color(p.text_dim));
                    ui.add_space(8.0);
                    for option in Theme::ALL {
                        let selected = self.store.data.settings.theme == option;
                        let name = match option {
                            Theme::Dark => lang.pick("Dark", "深色"),
                            Theme::Light => lang.pick("Light", "浅色"),
                        };
                        if ui.selectable_label(selected, name).clicked() {
                            self.store.data.settings.theme = option;
                            changed = true;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lang.pick("Language", "语言")).color(p.text_dim));
                    ui.add_space(8.0);
                    for option in Lang::ALL {
                        let selected = self.store.data.settings.language == option;
                        if ui.selectable_label(selected, option.native_name()).clicked() {
                            self.store.data.settings.language = option;
                            changed = true;
                        }
                    }
                });

                ui.add_space(14.0);
                ui.label(RichText::new(lang.pick("Sessions", "会话")).strong().color(p.text));
                ui.add_space(6.0);
                if ui
                    .checkbox(
                        &mut self.store.data.settings.view_only_default,
                        lang.pick("Start new sessions read-only", "新建会话默认只读"),
                    )
                    .changed()
                {
                    changed = true;
                }

                ui.add_space(14.0);
                let stored = RichText::new(lang.pick("Saved connections", "已保存的连接"))
                    .strong()
                    .color(p.text);
                ui.label(stored);
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        self.store.data.connections.len(),
                        lang.pick("stored", "个连接")
                    ))
                    .size(11.0)
                    .color(p.text_faint),
                );
                ui.add_space(6.0);
                #[cfg(not(target_os = "android"))]
                ui.horizontal(|ui| {
                    if ui.button(lang.pick("Import\u{2026}", "导入\u{2026}")).clicked() {
                        import = true;
                    }
                    if ui.button(lang.pick("Export\u{2026}", "导出\u{2026}")).clicked() {
                        export = true;
                    }
                });

                ui.add_space(14.0);
                ui.label(RichText::new(lang.pick("Config file", "配置文件")).strong().color(p.text));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(self.store.path().display().to_string())
                        .monospace()
                        .size(11.0)
                        .color(p.text_faint),
                );
            });

        self.settings_open = open;
        if changed {
            self.save();
        }
        #[cfg(not(target_os = "android"))]
        if import {
            self.import_connections();
        }
        #[cfg(not(target_os = "android"))]
        if export {
            self.export_connections();
        }
    }
    fn delete_dialog(&mut self, ctx: &Context, theme: Theme) {
        let Some(id) = self.confirm_delete else {
            return;
        };
        let Some(connection) = self.store.get(id) else {
            self.confirm_delete = None;
            return;
        };
        let p = palette(theme);
        let lang = self.lang();
        let name = connection.display_name().to_owned();
        let address = connection.address();

        let mut cancel = false;
        let mut delete = false;
        egui::Window::new(lang.pick("Delete connection", "删除连接"))
            .id(Id::new("airframe-delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(360.0)
            .frame(dialog_frame(theme))
            .show(ctx, |ui| {
                let title = match lang {
                    Lang::En => format!("Delete \u{201c}{name}\u{201d}?"),
                    Lang::Zh => format!("删除 \u{201c}{name}\u{201d}？"),
                };
                ui.label(RichText::new(title).strong().color(p.text));
                ui.add_space(4.0);
                ui.label(RichText::new(address).size(11.0).color(p.text_faint));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(lang.pick(
                        "A session already using it keeps running.",
                        "正在使用它的会话会继续运行。",
                    ))
                    .size(11.0)
                    .color(p.text_faint),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(lang.pick("Cancel", "取消")).clicked() {
                        cancel = true;
                    }
                    let label = RichText::new(lang.pick("Delete", "删除")).color(Color32::WHITE);
                    let remove = egui::Button::new(label)
                        .fill(p.danger)
                        .corner_radius(CornerRadius::same(6));
                    if ui.add(remove).clicked() {
                        delete = true;
                    }
                });
            });

        if delete {
            self.store.remove(id);
            self.run_passwords.remove(&id);
            if self.selection == Some(id) {
                self.selection = None;
            }
            self.save();
            let text = lang.pick("Connection deleted", "连接已删除");
            self.notify(text.into(), Notice::Info);
            self.confirm_delete = None;
        } else if cancel {
            self.confirm_delete = None;
        }
    }

    /// Send text into the remote session, key by key.
    ///
    /// The protocols carry key codes, so a remote desktop cannot be handed a
    /// paste. Typing the text is what makes a console with no clipboard of its
    /// own — a QEMU serial console, a bare Linux tty — usable.
    fn send_text_dialog(&mut self, ctx: &Context, theme: Theme) {
        let p = palette(theme);
        let lang = self.lang();
        let Some(buffer) = self.send_text.as_mut() else {
            return;
        };
        let mut send = false;
        let mut cancel = false;
        egui::Window::new(lang.pick("Send text", "发送文本"))
            .id(Id::new("airframe-send-text"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(440.0)
            .frame(dialog_frame(theme))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(lang.pick(
                        "Typed into the remote one key at a time; a newline presses Enter. Only characters a US keyboard can produce can be sent.",
                        "文本会逐键输入到远端，换行相当于按下 Enter。只能发送美式键盘能输入的字符。",
                    ))
                    .size(11.0)
                    .color(p.text_faint),
                );
                ui.add_space(8.0);
                let editor = ui.add(
                    egui::TextEdit::multiline(buffer)
                        .desired_rows(6)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .hint_text(lang.pick("Ctrl+V pastes here", "在此按 Ctrl+V 粘贴")),
                );
                if buffer.is_empty() {
                    // An empty box is all there is to do here, so it holds the
                    // focus and a paste lands in it without a click.
                    editor.request_focus();
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(lang.pick("Cancel", "取消")).clicked() {
                        cancel = true;
                    }
                    let ready = !buffer.trim().is_empty();
                    let label = RichText::new(lang.pick("Send", "发送")).color(Color32::WHITE);
                    let button = egui::Button::new(label)
                        .fill(p.accent)
                        .corner_radius(CornerRadius::same(6));
                    if ui.add_enabled(ready, button).clicked() {
                        send = true;
                    }
                });
            });

        if send {
            let text = self.send_text.take().unwrap_or_default();
            // Nothing to type into if the session went away with the dialog open.
            let Some(tab) = self.tabs.get_mut(self.active) else {
                return;
            };
            let skipped = tab.session.type_text(&text);
            // Saying what could not be typed matters: a remote keyboard has no
            // way to send anything outside its layout, and silence would look
            // like the paste had worked.
            if skipped == 0 {
                let message =
                    lang.pick("Typing into the remote session", "正在输入到远端");
                self.notify(message.into(), Notice::Success);
            } else {
                let message = match lang {
                    Lang::Zh => format!("正在输入到远端，有 {skipped} 个字符无法输入"),
                    Lang::En => format!("Typing into the remote; {skipped} character(s) cannot be typed"),
                };
                self.notify(message, Notice::Info);
            }
        } else if cancel {
            self.send_text = None;
        }
    }

    fn password_prompt_dialog(&mut self, ctx: &Context, theme: Theme) {
        let p = palette(theme);
        let lang = self.lang();
        let Some(prompt) = self.password_prompt.as_mut() else {
            return;
        };
        let mut connect = false;
        let mut cancel = false;
        egui::Window::new(lang.pick("Password required", "需要密码"))
            .id(Id::new("airframe-password-prompt"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(400.0)
            .frame(dialog_frame(theme))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(lang.pick(
                        "This server requires a password. Enter it below to connect.",
                        "此服务器需要密码。在下方输入密码以连接。",
                    ))
                    .size(12.0)
                    .color(p.text_faint),
                );
                ui.add_space(10.0);
                let _ = ui.add(
                    egui::TextEdit::singleline(&mut prompt.buffer)
                        .password(true)
                        .desired_width(f32::INFINITY)
                        .hint_text(lang.pick("Password", "密码")),
                );
                ui.add_space(8.0);
                ui.checkbox(
                    &mut prompt.remember,
                    lang.pick("Remember on this device", "在此设备上记住"),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(lang.pick("Cancel", "取消")).clicked() {
                        cancel = true;
                    }
                    let label = RichText::new(lang.pick("Connect", "连接")).color(Color32::WHITE);
                    let button = egui::Button::new(label)
                        .fill(p.accent)
                        .corner_radius(CornerRadius::same(6));
                    if ui.add_enabled(!prompt.buffer.is_empty(), button).clicked() {
                        connect = true;
                    }
                });
            });

        if connect {
            let password = self
                .password_prompt
                .as_ref()
                .map(|p| p.buffer.clone())
                .unwrap_or_default();
            let remember = self
                .password_prompt
                .as_ref()
                .map(|p| p.remember)
                .unwrap_or(false);
            let tab_index = self.password_prompt.as_ref().map(|p| p.tab).unwrap_or(0);
            let connection = self
                .tabs
                .get_mut(tab_index)
                .map(|tab| {
                    tab.session.connection.password = Some(password.clone());
                    tab.session.needs_password = false;
                    if remember {
                        tab.session.connection.remember_password = true;
                    }
                    tab.session.connection.clone()
                });
            if let Some(connection) = connection {
                if remember {
                    self.store.upsert(connection);
                    self.save();
                } else {
                    self.run_passwords.insert(connection.id, password.clone());
                }
            }
            if let Some(tab) = self.tabs.get_mut(tab_index) {
                tab.session.restart();
            }
            self.password_prompt = None;
        } else if cancel {
            let tab_index = self.password_prompt.as_ref().map(|p| p.tab).unwrap_or(0);
            if let Some(tab) = self.tabs.get_mut(tab_index) {
                tab.session.needs_password = false;
            }
            self.password_prompt = None;
        }
    }

    /// Connections are exported without their passwords, so a shared file never
    /// leaks a credential the user only meant to keep locally.
    ///
    /// Only offered where a native file dialog exists; Android has neither, so
    /// the buttons are hidden there too.
    #[cfg(not(target_os = "android"))]
    fn export_connections(&mut self) {
        let lang = self.lang();
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("airframe-connections.json")
            .add_filter(lang.pick("AirFrame connections", "AirFrame 连接"), &["json"])
            .save_file()
        else {
            return;
        };
        let mut connections = self.store.data.connections.clone();
        for connection in &mut connections {
            connection.password = None;
        }
        match serde_json::to_string_pretty(&connections) {
            Ok(text) => match std::fs::write(&path, text) {
                Ok(()) => {
                    let text = format!("{} {}", lang.pick("Exported to", "已导出到"), path.display());
                    self.notify(text, Notice::Success)
                }
                Err(error) => {
                    let text = format!(
                        "{} {}: {error}",
                        lang.pick("Could not write", "无法写入"),
                        path.display()
                    );
                    self.notify(text, Notice::Error)
                }
            },
            Err(error) => {
                let text =
                    format!("{}: {error}", lang.pick("Could not encode connections", "无法编码连接数据"));
                self.notify(text, Notice::Error)
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    fn import_connections(&mut self) {
        let lang = self.lang();
        let Some(path) = rfd::FileDialog::new()
            .add_filter(lang.pick("AirFrame connections", "AirFrame 连接"), &["json"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                let text = format!(
                    "{} {}: {error}",
                    lang.pick("Could not read", "无法读取"),
                    path.display()
                );
                self.notify(text, Notice::Error);
                return;
            }
        };
        match serde_json::from_str::<Vec<Connection>>(&text) {
            Ok(connections) => {
                let count = connections.len();
                for connection in connections {
                    self.store.upsert(connection);
                }
                self.save();
                let text = match lang {
                    Lang::En => format!("Imported {count} connection(s)"),
                    Lang::Zh => format!("已导入 {count} 个连接"),
                };
                self.notify(text, Notice::Success);
            }
            Err(error) => {
                let text = format!(
                    "{}: {error}",
                    lang.pick("Not an AirFrame connection file", "不是 AirFrame 连接文件")
                );
                self.notify(text, Notice::Error)
            }
        }
    }

    fn draw_toast(&mut self, ctx: &Context) {
        let expired = self.toast.as_ref().is_some_and(|toast| Instant::now() >= toast.until);
        if expired {
            self.toast = None;
            return;
        }
        let Some(toast) = self.toast.clone() else {
            return;
        };
        let p = palette(self.theme());
        let color = match toast.notice {
            Notice::Info => p.text,
            Notice::Success => p.success,
            Notice::Error => p.danger,
        };
        Area::new(Id::new("airframe-toast"))
            .order(Order::Foreground)
            .interactable(false)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -22.0))
            .show(ctx, |ui| {
                Frame {
                    fill: p.elevated,
                    corner_radius: CornerRadius::same(10),
                    inner_margin: Margin::symmetric(14, 10),
                    shadow: egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(90),
                    },
                    ..Frame::NONE
                }
                .show(ui, |ui| {
                    ui.label(RichText::new(&toast.text).color(color).size(12.5));
                });
            });
        ctx.request_repaint_after(Duration::from_millis(120));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    /// Leave before macOS can tear its Touch Bar support down.
    ///
    /// AppKit removes the Touch Bar finder's observation of the window after
    /// the window is gone, and on Touch Bar MacBooks it throws when doing so —
    /// its finder removing an observer it no longer has — which the crash
    /// reporter shows as a crash on quit. Exiting here keeps that off the
    /// user's screen; eframe has already saved by this point, and the config
    /// store is written on every change, so nothing is lost.
    ///
    /// Android builds use eframe's glow renderer, whose `on_exit` passes the
    /// glow context; the two signatures below are otherwise the same method.
    #[cfg(target_os = "android")]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}

    #[cfg(not(target_os = "android"))]
    fn on_exit(&mut self) {
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

impl App {
    /// Draw the entire window into `ui`.
    ///
    /// Kept separate from the thin [`eframe::App`] adapter above so the UI can be
    /// driven headlessly through `egui_kittest`.
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let theme = self.theme();
        let lang = self.lang();
        if self.applied_theme != Some(theme) {
            ui_theme::apply(&ctx, theme);
            self.applied_theme = Some(theme);
        }

        self.shortcuts(&ctx);
        self.pump(&ctx);

        let p = palette(theme);
        // The chrome is one L-shaped surface with seams against the content,
        // rather than three boxes each drawn with a border all the way round.
        let top = Panel::top("airframe-topbar")
            .frame(Frame {
                fill: p.panel,
                inner_margin: Margin::symmetric(12, 7),
                ..Frame::NONE
            })
            .show(ui, |ui| self.top_bar(ui));

        let side = Panel::left("airframe-sidebar")
            .frame(Frame {
                fill: p.panel,
                inner_margin: Margin::symmetric(10, 8),
                ..Frame::NONE
            })
            .resizable(true)
            .default_size(278.0)
            .size_range(220.0..=460.0)
            .show(ui, |ui| sidebar::show(ui, self));

        CentralPanel::default()
            .frame(Frame { fill: p.bg, ..Frame::NONE })
            .show(ui, |ui| self.workspace(ui, theme, lang));

        let seam = Stroke::new(1.0, p.border);
        ui.painter().hline(top.response.rect.x_range(), top.response.rect.bottom() - 0.5, seam);
        ui.painter().vline(side.response.rect.right() - 0.5, side.response.rect.y_range(), seam);

        self.dialogs(&ctx);
        self.draw_toast(&ctx);
        self.apply_actions(&ctx);
    }
}

/// The frame every dialog shares.
pub(crate) fn dialog_frame(theme: Theme) -> Frame {
    let p = palette(theme);
    Frame {
        fill: p.elevated,
        corner_radius: CornerRadius::same(14),
        inner_margin: Margin::same(16),
        shadow: egui::epaint::Shadow {
            offset: [0, 10],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(120),
        },
        ..Frame::NONE
    }
}

/// Where screenshots go: the user's pictures folder when the platform has one,
/// otherwise beside the connection store.
fn screenshot_dir(store_path: &Path) -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let pictures = PathBuf::from(home).join("Pictures");
        if pictures.is_dir() {
            return pictures;
        }
    }
    store_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("screenshots")
}

/// A file name for `name` that will not upset a filesystem.
fn screenshot_stem(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' {
            stem.push(c);
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }
    match stem.trim_matches('-') {
        "" => "airframe-session".to_owned(),
        stem => format!("airframe-{stem}"),
    }
}

/// `<stem>.png` under `directory`, numbered if that name is taken.
fn unique_path(directory: &Path, stem: &str) -> PathBuf {
    let first = directory.join(format!("{stem}.png"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| directory.join(format!("{stem}-{n}.png")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_names_are_tidy_and_unique() {
        assert_eq!(screenshot_stem("Build server"), "airframe-Build-server");
        assert_eq!(screenshot_stem("生产环境 / 1"), "airframe-生产环境-1");
        assert_eq!(screenshot_stem("  "), "airframe-session");

        let directory = std::env::temp_dir().join("airframe-screenshot-names");
        std::fs::create_dir_all(&directory).expect("a scratch directory");
        std::fs::write(directory.join("airframe-lab.png"), b"").expect("an existing shot");
        assert_eq!(unique_path(&directory, "airframe-lab"), directory.join("airframe-lab-2.png"));
        assert_eq!(unique_path(&directory, "airframe-new"), directory.join("airframe-new.png"));
        std::fs::remove_dir_all(&directory).ok();
    }
}
