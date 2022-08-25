use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::macros::ok_or_return;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ephemeral: bool,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let content = ok_or_return!(fs::read_to_string(path), Self::default());
        ok_or_return!(toml::from_str(&content), Self::default())
    }
}
