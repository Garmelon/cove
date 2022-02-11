use serde::{Deserialize, Serialize};

use crate::{Identity, SessionId};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Session {
    pub id: SessionId,
    pub nick: String,
    pub identity: Identity,
}
