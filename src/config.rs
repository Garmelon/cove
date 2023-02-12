use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::macros::ok_or_return;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomsSortOrder {
    #[default]
    Alphabet,
    Importance,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EuphRoom {
    // TODO Mark favourite rooms via printable ascii characters
    #[serde(default)]
    pub autojoin: bool,
    pub username: Option<String>,
    #[serde(default)]
    pub force_username: bool,
    pub password: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Euph {
    pub rooms: HashMap<String, EuphRoom>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub rooms_sort_order: RoomsSortOrder,
    #[serde(default)]
    pub timezone_offset: Option<String>,
    // TODO Invoke external notification command?
    pub euph: Euph,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let content = ok_or_return!(fs::read_to_string(path), Self::default());
        match toml::from_str(&content) {
            Ok(config) => config,
            Err(err) => {
                println!("Error loading config file: {err}");
                Self::default()
            }
        }
    }

    pub fn euph_room(&self, name: &str) -> EuphRoom {
        self.euph.rooms.get(name).cloned().unwrap_or_default()
    }

    pub fn timezone_offset(&self) -> time::UtcOffset {
        log::info!("timezone_offset: {:?}", self.timezone_offset);

        let default = time::UtcOffset::UTC;
        if self.timezone_offset.is_none() {
            return default;
        }

        let timezone_offset = self.timezone_offset.as_ref().unwrap();

        // Convert to time::UtcOffset
        // The string is in the format of "hours.minutes.seconds"
        // Where hours is a signed integer, and minutes and seconds are unsigned integers

        // Split the string into hours, minutes, and seconds
        let split: Vec<_> = timezone_offset.split(".").collect();

        let hours = split[0].parse::<i8>().unwrap();
        let minutes = split[1].parse::<i8>().unwrap();
        let seconds = split[2].parse::<i8>().unwrap();

        time::UtcOffset::from_hms(hours, minutes, seconds).unwrap_or(default)
    }
}
