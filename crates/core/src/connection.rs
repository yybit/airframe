//! The connection model: what a saved remote-desktop entry looks like on disk.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The remote-desktop protocol used to reach a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vnc,
    Rdp,
    Spice,
}

impl Protocol {
    pub const ALL: [Protocol; 3] = [Protocol::Vnc, Protocol::Rdp, Protocol::Spice];

    pub fn label(self) -> &'static str {
        match self {
            Protocol::Vnc => "VNC",
            Protocol::Rdp => "RDP",
            Protocol::Spice => "SPICE",
        }
    }

    /// The TCP port a server of this kind listens on by default.
    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Vnc => 5900,
            Protocol::Rdp => 3389,
            Protocol::Spice => 5930,
        }
    }

    /// URL scheme, used for sharing and importing connections.
    pub fn scheme(self) -> &'static str {
        match self {
            Protocol::Vnc => "vnc",
            Protocol::Rdp => "rdp",
            Protocol::Spice => "spice",
        }
    }
}

/// How RDP negotiates transport security.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RdpSecurity {
    /// Offer NLA first, then TLS, then plain RDP.
    #[default]
    Automatic,
    /// TLS only, no CredSSP/NLA pre-authentication.
    Tls,
    /// CredSSP/NLA pre-authentication over TLS.
    Nla,
}

impl RdpSecurity {
    pub const ALL: [RdpSecurity; 3] = [RdpSecurity::Automatic, RdpSecurity::Tls, RdpSecurity::Nla];

    pub fn label(self) -> &'static str {
        match self {
            RdpSecurity::Automatic => "Automatic",
            RdpSecurity::Tls => "TLS",
            RdpSecurity::Nla => "NLA",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VncOptions {
    /// Ask the server to share this session with other clients instead of
    /// disconnecting them.
    pub shared: bool,
}

impl Default for VncOptions {
    fn default() -> Self {
        Self { shared: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RdpOptions {
    /// Allow RDP bulk compression.
    pub compression: bool,
    pub security: RdpSecurity,
    /// Remote desktop size. Zero means "follow the viewport".
    pub width: u16,
    pub height: u16,
}

impl Default for RdpOptions {
    fn default() -> Self {
        Self {
            compression: true,
            security: RdpSecurity::Automatic,
            width: 0,
            height: 0,
        }
    }
}

/// A saved remote-desktop entry.
///
/// Passwords are only written to disk when [`Connection::remember_password`]
/// is set; otherwise the password lives in memory for the current run only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    /// RDP domain, or a VNC/SPICE realm hint.
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub view_only: bool,
    #[serde(default)]
    pub remember_password: bool,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub vnc: VncOptions,
    #[serde(default)]
    pub rdp: RdpOptions,
}

impl Connection {
    /// A new entry with everything at its default.
    pub fn new(name: impl Into<String>, protocol: Protocol, host: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            protocol,
            host: host.into(),
            port: protocol.default_port(),
            username: String::new(),
            domain: String::new(),
            group: String::new(),
            view_only: false,
            remember_password: false,
            password: None,
            vnc: VncOptions::default(),
            rdp: RdpOptions::default(),
        }
    }

    /// `host:port`, as it is handed to the protocol backends.
    pub fn address(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            // Bare IPv6 literal.
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// The name to show in the tab bar and window title.
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.host
        } else {
            &self.name
        }
    }

    /// Drop the password if the user did not ask for it to be remembered.
    pub fn scrub_password(&mut self) {
        if !self.remember_password {
            self.password = None;
        }
    }
}
