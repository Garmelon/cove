use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::doc::Document;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Document)]
#[serde(rename_all = "snake_case")]
pub enum RoomsSortOrder {
    #[default]
    Alphabet,
    Importance,
}

// TODO Mark favourite rooms via printable ascii characters
#[derive(Debug, Clone, Default, Deserialize, Document)]
pub struct EuphRoom {
    /// Whether to automatically join this room on startup.
    #[serde(default)]
    pub autojoin: bool,

    /// If set, cove will set this username upon joining if there is no username
    /// associated with the current session.
    pub username: Option<String>,

    /// If `euph.servers.<domain>.rooms.<room>.username` is set, this will force
    /// cove to set the username even if there is already a different username
    /// associated with the current session.
    #[serde(default)]
    pub force_username: bool,

    /// If set, cove will try once to use this password to authenticate, should
    /// the room be password-protected.
    pub password: Option<String>,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct EuphServer {
    #[document(metavar = "room")]
    pub rooms: HashMap<String, EuphRoom>,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Euph {
    #[document(metavar = "domain")]
    pub servers: HashMap<String, EuphServer>,
}
