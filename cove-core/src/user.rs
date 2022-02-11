use serde::{Deserialize, Serialize};

use crate::{Identity, SessionId};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub nick: String,
    pub identity: Identity,
    pub sid: SessionId,
}
