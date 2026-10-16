//! Saved connections and preferences, persisted as JSON under the user's
//! config directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::connection::Connection;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("could not parse {path}: {source}")]
    Parse { path: PathBuf, source: serde_json::Error },
    #[error("could not write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub const ALL: [Theme; 2] = [Theme::Dark, Theme::Light];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
        }
    }
}

/// The language the interface is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Zh];

    /// The language's own name, for the picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "中文",
        }
    }

    /// The interface language to start in: whatever the OS locale asks for.
    pub fn from_env() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key)
                && !value.is_empty()
            {
                return if value.starts_with("zh") { Lang::Zh } else { Lang::En };
            }
        }
        Lang::En
    }

    /// The string for this language.
    pub fn pick(self, en: &'static str, zh: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::Zh => zh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Start every new session read-only.
    pub view_only_default: bool,
    /// The language the interface is written in.
    pub language: Lang,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Theme::Dark, view_only_default: false, language: Lang::from_env() }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StoreData {
    pub connections: Vec<Connection>,
    pub settings: Settings,
}

/// The on-disk store.
pub struct Store {
    path: PathBuf,
    pub data: StoreData,
}

impl Store {
    /// Load from the platform's config directory, or start empty when there is
    /// nothing there yet.
    pub fn load() -> (Self, Option<StoreError>) {
        Self::load_from(default_config_path())
    }

    pub fn load_from(path: PathBuf) -> (Self, Option<StoreError>) {
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<StoreData>(&text) {
                Ok(data) => (Self { path, data }, None),
                Err(source) => {
                    let err = StoreError::Parse { path: path.clone(), source };
                    (Self { path, data: StoreData::default() }, Some(err))
                }
            },
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                (Self { path, data: StoreData::default() }, None)
            }
            Err(source) => {
                let err = StoreError::Read { path: path.clone(), source };
                (Self { path, data: StoreData::default() }, Some(err))
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the store back, atomically and with owner-only permissions.
    pub fn save(&self) -> Result<(), StoreError> {
        let write = || -> io::Result<()> {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let text = serde_json::to_string_pretty(&self.data).unwrap_or_else(|_| "{}".into());
            let tmp = self.path.with_extension("json.tmp");
            fs::write(&tmp, text.as_bytes())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
            }
            fs::rename(&tmp, &self.path)
        };
        write().map_err(|source| StoreError::Write { path: self.path.clone(), source })
    }

    pub fn get(&self, id: Uuid) -> Option<&Connection> {
        self.data.connections.iter().find(|c| c.id == id)
    }

    /// Insert or replace by id.
    pub fn upsert(&mut self, mut connection: Connection) {
        connection.scrub_password();
        match self.data.connections.iter_mut().find(|c| c.id == connection.id) {
            Some(existing) => *existing = connection,
            None => self.data.connections.push(connection),
        }
    }

    pub fn remove(&mut self, id: Uuid) {
        self.data.connections.retain(|c| c.id != id);
    }
}

/// Where the saved connections live.
///
/// `$XDG_CONFIG_HOME/airframe/airframe.json` — falling back to
/// `~/.config/airframe/airframe.json` — on Unix, and
/// `~/Library/Application Support/AirFrame/airframe.json` on macOS.
pub fn default_config_path() -> PathBuf {
    let home = || std::env::var_os("HOME").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let base = home().map(|home| home.join("Library/Application Support"));
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|home| home.join(".config")));

    base.unwrap_or_else(|| PathBuf::from("."))
        .join(config_dir())
        .join("airframe.json")
}

/// The app's own directory under the platform's config root. macOS names it
/// after the app; the freedesktop convention wants it lowercase.
#[cfg(target_os = "macos")]
fn config_dir() -> &'static str {
    "AirFrame"
}

#[cfg(not(target_os = "macos"))]
fn config_dir() -> &'static str {
    "airframe"
}
