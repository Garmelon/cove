use std::collections::HashMap;

use serde::Deserialize;

use crate::doc::{Doc, Document};

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
