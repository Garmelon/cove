use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::macros::ok_or_return;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub ephemeral: bool,
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
}
