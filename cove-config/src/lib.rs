#![forbid(unsafe_code)]
// Rustc lint groups
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]
#![warn(unused)]
// Rustc lints
#![warn(noop_method_call)]
#![warn(single_use_lifetimes)]
// Clippy lints
#![warn(clippy::use_self)]

pub mod doc;
mod euph;
mod keys;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{fs, io};

use doc::Document;
use serde::Deserialize;

pub use crate::euph::*;
pub use crate::keys::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read config file")]
    Io(#[from] io::Error),
    #[error("failed to parse config file")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Config {
    /// The directory that cove stores its data in when not running in ephemeral
    /// mode.
    ///
    /// Relative paths are interpreted relative to the user's home directory.
    ///
    /// See also the `--data-dir` command line option.
    #[document(default = "platform-dependent")]
    pub data_dir: Option<PathBuf>,

    /// Whether to start in ephemeral mode.
    ///
    /// In ephemeral mode, cove doesn't store any data. It completely ignores
    /// any options related to the data dir.
    ///
    /// See also the `--ephemeral` command line option.
    #[serde(default)]
    #[document(default = "`false`")]
    pub ephemeral: bool,

    /// Whether to measure the width of characters as displayed by the terminal
    /// emulator instead of guessing the width.
    ///
    /// Enabling this makes rendering a bit slower but more accurate. The screen
    /// might also flash when encountering new characters (or, more accurately,
    /// graphemes).
    ///
    /// See also the `--measure-graphemes` command line option.
    #[serde(default)]
    #[document(default = "`false`")]
    pub measure_widths: bool,

    /// Whether to start in offline mode.
    ///
    /// In offline mode, cove won't automatically join rooms marked via the
    /// `autojoin` option on startup. You can still join those rooms manually by
    /// pressing `a` in the rooms list.
    ///
    /// See also the `--offline` command line option.
    #[serde(default)]
    #[document(default = "`false`")]
    pub offline: bool,

    /// Initial sort order of rooms list.
    ///
    /// `alphabet` sorts rooms in alphabetic order.
    ///
    /// `importance` sorts rooms by the following criteria (in descending order
    /// of priority):
    ///
    /// 1. connected rooms before unconnected rooms
    /// 2. rooms with unread messages before rooms without
    /// 3. alphabetic order
    #[serde(default)]
    #[document(default = "`alphabet`")]
    pub rooms_sort_order: RoomsSortOrder,

    #[serde(default)]
    #[document(no_default)]
    pub euph: Euph,

    #[serde(default)]
    #[document(no_default)]
    pub keys: Keys,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        Ok(match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content)?,
            Err(err) if err.kind() == ErrorKind::NotFound => Self::default(),
            Err(err) => Err(err)?,
        })
    }

    pub fn euph_room(&self, domain: &str, name: &str) -> EuphRoom {
        if let Some(server) = self.euph.servers.get(domain) {
            if let Some(room) = server.rooms.get(name) {
                return room.clone();
            }
        }
        EuphRoom::default()
    }
}
