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
    pub ephemeral: bool,

    /// Whether to measure the width of characters as displayed by the terminal
    /// emulator instead of guessing the width.
    ///
    /// Enabling this makes rendering a bit slower but more accurate. The screen
    /// might also flash when encountering new characters (or, more accurately,
    /// graphemes).
    ///
    /// See also the `--measure-widths` command line option.
    #[serde(default)]
    pub measure_widths: bool,

    /// Whether to start in offline mode.
    ///
    /// In offline mode, cove won't automatically join rooms marked via the
    /// `autojoin` option on startup. You can still join those rooms manually by
    /// pressing `a` in the rooms list.
    ///
    /// See also the `--offline` command line option.
    #[serde(default)]
    pub offline: bool,

    /// Initial sort order of rooms list.
    ///
    /// `"alphabet"` sorts rooms in alphabetic order.
    ///
    /// `"importance"` sorts rooms by the following criteria (in descending
    /// order of priority):
    ///
    /// 1. connected rooms before unconnected rooms
    /// 2. rooms with unread messages before rooms without
    /// 3. alphabetic order
    #[serde(default)]
    pub rooms_sort_order: RoomsSortOrder,

    /// Time zone that chat timestamps should be displayed in.
    ///
    /// This option can either be the string `"localtime"`, a [POSIX TZ string],
    /// or a [tz identifier] from the [tz database].
    ///
    /// When not set or when set to `"localtime"`, cove attempts to use your
    /// system's configured time zone, falling back to UTC.
    ///
    /// When the string begins with a colon or doesn't match the a POSIX TZ
    /// string format, it is interpreted as a tz identifier and looked up in
    /// your system's tz database (or a bundled tz database on Windows).
    ///
    /// If the `TZ` environment variable exists, it overrides this option.
    ///
    /// [POSIX TZ string]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html#tag_08_03
    /// [tz identifier]: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones
    /// [tz database]: https://en.wikipedia.org/wiki/Tz_database
    #[serde(default)]
    #[document(default = "`$TZ` or local system time zone")]
    pub time_zone: Option<String>,

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

    pub fn time_zone_ref(&self) -> Option<&str> {
        self.time_zone.as_ref().map(|s| s as &str)
    }
}
