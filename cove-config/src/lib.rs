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

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use cove_macro::Document;
use doc::{Doc, Document};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomsSortOrder {
    #[default]
    Alphabet,
    Importance,
}

impl Document for RoomsSortOrder {
    fn doc() -> Doc {
        let mut doc = String::doc();
        doc.value_info.values = Some(vec![
            // TODO Generate by serializing
            "`alphabet`".to_string(),
            "`importance`".to_string(),
        ]);
        doc
    }
}

// TODO Mark favourite rooms via printable ascii characters
#[derive(Debug, Clone, Default, Deserialize, Document)]
pub struct EuphRoom {
    /// Whether to automatically join this room on startup.
    #[serde(default)]
    #[document(default = "`false`")]
    pub autojoin: bool,

    /// If set, cove will set this username upon joining if there is no username
    /// associated with the current session.
    pub username: Option<String>,

    /// If `euph.rooms.<room>.username` is set, this will force cove to set the
    /// username even if there is already a different username associated with
    /// the current session.
    #[serde(default)]
    #[document(default = "`false`")]
    pub force_username: bool,

    /// If set, cove will try once to use this password to authenticate, should
    /// the room be password-protected.
    pub password: Option<String>,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Euph {
    #[document(metavar = "room")]
    pub rooms: HashMap<String, EuphRoom>,
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

    // TODO Invoke external notification command?
    pub euph: Euph,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let Ok(content) = fs::read_to_string(path) else { return Self::default(); };
        match toml::from_str(&content) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error loading config file: {err}");
                Self::default()
            }
        }
    }

    pub fn euph_room(&self, name: &str) -> EuphRoom {
        self.euph.rooms.get(name).cloned().unwrap_or_default()
    }
}
