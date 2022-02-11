use serde::{Deserialize, Serialize};

use crate::{Identity, SessionId};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    nick: String,
    identity: Identity,
    sid: SessionId,
}
