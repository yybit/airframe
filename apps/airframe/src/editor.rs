//! The connection editor: one form for creating and changing a saved entry.

use airframe_core::{Connection, Lang, Protocol, RdpSecurity, Theme};
use airframe_ui::palette;
use egui::{Align2, Context, CornerRadius, DragValue, Grid, RichText, TextEdit, Ui, Vec2};
use uuid::Uuid;

/// What the user did with the editor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Save { connect: bool },
    Cancel,
}

pub struct Editor {
    pub connection: Connection,
    /// The password being edited, kept apart from the stored entry so the
    /// "remember" policy is applied exactly once, on save.
    pub password: String,
    /// True when the entry already exists in the store.
    pub existing: bool,
    /// Whether saving should also open a session.
    pub connect: bool,
    pub reveal: bool,
    pub error: Option<String>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            connection: Connection::new(String::new(), Protocol::Vnc, "localhost"),
            password: String::new(),
            existing: false,
            connect: true,
            reveal: false,
            error: None,
        }
    }

    pub fn edit(connection: &Connection) -> Self {
        Self {
            connection: connection.clone(),
            password: connection.password.clone().unwrap_or_default(),
            existing: true,
            connect: false,
            reveal: false,
            error: None,
        }
    }

    pub fn duplicate(connection: &Connection, lang: Lang) -> Self {
        let mut copy = connection.clone();
        copy.id = Uuid::new_v4();
        copy.name = if connection.name.trim().is_empty() {
            String::new()
        } else {
            format!("{} {}", connection.name.trim(), lang.pick("copy", "副本"))
        };
        Self {
            password: connection.password.clone().unwrap_or_default(),
            connection: copy,
            existing: false,
            connect: false,
            reveal: false,
            error: None,
        }
    }

    /// Move the edited password into the connection under the remember policy.
    fn finish(&mut self) {
        self.connection.password = match self.password.is_empty() {
            true => None,
            false => Some(self.password.clone()),
        };
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

pub fn show(ctx: &Context, editor: &mut Editor, theme: Theme, lang: Lang) -> Option<Outcome> {
    let p = palette(theme);
    let title = if editor.existing {
        lang.pick("Edit connection", "编辑连接")
    } else {
        lang.pick("New connection", "新建连接")
    };

    let mut outcome = None;
    let mut open = true;

    egui::Window::new(title)
        .id(egui::Id::new("airframe-editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .default_width(470.0)
        .frame(crate::app::dialog_frame(theme))
        .show(ctx, |ui| {
            Grid::new("airframe-editor-grid")
                .num_columns(2)
                .spacing([16.0, 9.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(lang.pick("Name", "名称")).color(p.text_dim));
                    ui.add(
                        TextEdit::singleline(&mut editor.connection.name)
                            .hint_text(lang.pick("Office desktop", "办公室电脑"))
                            .desired_width(292.0),
                    );
                    ui.end_row();

                    ui.label(RichText::new(lang.pick("Protocol", "协议")).color(p.text_dim));
                    ui.horizontal(|ui| {
                        let previous = editor.connection.protocol;
                        for protocol in Protocol::ALL {
                            let selected = editor.connection.protocol == protocol;
                            if ui.selectable_label(selected, protocol.label()).clicked() {
                                editor.connection.protocol = protocol;
                            }
                        }
                        if editor.connection.protocol != previous
                            && editor.connection.port == previous.default_port()
                        {
                            editor.connection.port = editor.connection.protocol.default_port();
                        }
                    });
                    ui.end_row();

                    ui.label(RichText::new(lang.pick("Host", "主机")).color(p.text_dim));
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut editor.connection.host)
                                .hint_text("10.0.0.5")
                                .desired_width(212.0),
                        );
                        ui.label(RichText::new(":").color(p.text_faint));
                        ui.add(
                            DragValue::new(&mut editor.connection.port)
                                .range(1..=65535)
                                .speed(1.0),
                        );
                    });
                    ui.end_row();

                    ui.label(RichText::new(lang.pick("User", "用户名")).color(p.text_dim));
                    ui.add(
                        TextEdit::singleline(&mut editor.connection.username)
                            .hint_text(lang.pick("optional", "可选"))
                            .desired_width(292.0),
                    );
                    ui.end_row();

                    if editor.connection.protocol == Protocol::Rdp {
                        ui.label(RichText::new(lang.pick("Domain", "域")).color(p.text_dim));
                        ui.add(
                            TextEdit::singleline(&mut editor.connection.domain)
                                .hint_text(lang.pick("optional", "可选"))
                                .desired_width(292.0),
                        );
                        ui.end_row();
                    }

                    let reveal = editor.reveal;
                    ui.label(RichText::new(lang.pick("Password", "密码")).color(p.text_dim));
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut editor.password)
                                .password(!reveal)
                                .hint_text(lang.pick("optional", "可选"))
                                .desired_width(212.0),
                        );
                        ui.checkbox(&mut editor.reveal, lang.pick("Show", "显示"));
                    });
                    ui.end_row();

                    ui.label("");
                    ui.vertical(|ui| {
                        ui.checkbox(
                            &mut editor.connection.remember_password,
                            lang.pick("Remember on this machine", "在此设备上记住"),
                        );
                        if !editor.connection.remember_password && !editor.password.is_empty() {
                            ui.label(
                                RichText::new(lang.pick("Used for this run only", "仅本次运行有效"))
                                    .size(10.5)
                                    .color(p.text_faint),
                            );
                        }
                    });
                    ui.end_row();

                    ui.label(RichText::new(lang.pick("Group", "分组")).color(p.text_dim));
                    ui.add(
                        TextEdit::singleline(&mut editor.connection.group)
                            .hint_text(lang.pick("optional", "可选"))
                            .desired_width(292.0),
                    );
                    ui.end_row();

                    ui.label(RichText::new(lang.pick("Access", "访问")).color(p.text_dim));
                    let label = lang.pick("View only (send no input)", "只读查看（不发送输入）");
                    ui.checkbox(&mut editor.connection.view_only, label);
                    ui.end_row();

                    options(ui, editor, theme, lang);
                });

            if let Some(error) = &editor.error {
                ui.add_space(10.0);
                ui.label(RichText::new(error).color(p.danger).size(12.0));
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if editor.connect {
                    lang.pick("Save & connect", "保存并连接")
                } else {
                    lang.pick("Save", "保存")
                };
                let primary = egui::Button::new(RichText::new(label).color(p.on_accent))
                    .fill(p.accent)
                    .corner_radius(CornerRadius::same(7))
                    .min_size(Vec2::new(126.0, 28.0));
                if ui.add(primary).clicked() {
                    match validate(editor, lang) {
                        Ok(()) => outcome = Some(Outcome::Save { connect: editor.connect }),
                        Err(message) => editor.error = Some(message),
                    }
                }
                if ui.button(lang.pick("Cancel", "取消")).clicked() {
                    outcome = Some(Outcome::Cancel);
                }
                if !editor.existing {
                    ui.add_space(6.0);
                    ui.checkbox(&mut editor.connect, lang.pick("Connect after saving", "保存后连接"));
                }
            });
        });

    if let Some(Outcome::Save { .. }) = &outcome {
        editor.finish();
    }
    if outcome.is_none() && !open {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

/// The protocol-specific half of the form.
fn options(ui: &mut Ui, editor: &mut Editor, theme: Theme, lang: Lang) {
    let p = palette(theme);
    match editor.connection.protocol {
        Protocol::Vnc => {
            ui.label(RichText::new(lang.pick("Sharing", "共享")).color(p.text_dim));
            let label = lang.pick("Let other viewers stay connected", "允许其他查看者保持连接");
            ui.checkbox(&mut editor.connection.vnc.shared, label);
            ui.end_row();
        }
        Protocol::Spice => {}
        Protocol::Rdp => {
            ui.label(RichText::new(lang.pick("Security", "安全")).color(p.text_dim));
            ui.horizontal(|ui| {
                for security in RdpSecurity::ALL {
                    let selected = editor.connection.rdp.security == security;
                    if ui.selectable_label(selected, security.label()).clicked() {
                        editor.connection.rdp.security = security;
                    }
                }
            });
            ui.end_row();

            ui.label(RichText::new(lang.pick("Compression", "压缩")).color(p.text_dim));
            let label = lang.pick("Allow bulk compression", "允许批量压缩");
            ui.checkbox(&mut editor.connection.rdp.compression, label);
            ui.end_row();

            ui.label(RichText::new(lang.pick("Desktop", "桌面")).color(p.text_dim));
            ui.horizontal(|ui| {
                let rdp = &mut editor.connection.rdp;
                let mut follow = rdp.width == 0 || rdp.height == 0;
                let label = lang.pick("Follow the window size", "跟随窗口大小");
                if ui.checkbox(&mut follow, label).changed() {
                    if follow {
                        rdp.width = 0;
                        rdp.height = 0;
                    } else {
                        (rdp.width, rdp.height) = (1280, 800);
                    }
                }
                if !follow {
                    ui.add(DragValue::new(&mut rdp.width).range(200..=8192));
                    ui.label(RichText::new("\u{d7}").color(p.text_faint));
                    ui.add(DragValue::new(&mut rdp.height).range(200..=8192));
                }
            });
            ui.end_row();
        }
    }
}

fn validate(editor: &Editor, lang: Lang) -> Result<(), String> {
    if editor.connection.host.trim().is_empty() {
        return Err(lang.pick("A host is required.", "必须填写主机地址。").into());
    }
    if editor.connection.host.contains(char::is_whitespace) {
        return Err(lang.pick("The host must not contain spaces.", "主机地址不能包含空格。").into());
    }
    if !(1..=65535).contains(&editor.connection.port) {
        return Err(
            lang.pick("The port must be between 1 and 65535.", "端口必须介于 1 和 65535 之间。").into()
        );
    }
    Ok(())
}
