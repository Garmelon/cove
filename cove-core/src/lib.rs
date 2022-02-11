mod id;
mod macros;
mod message;

use serde::{Deserialize, Serialize};

pub use self::id::*;
use self::macros::packets;
pub use self::message::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct HelloCmd {
    pub id: Id,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum HelloRpl {
    Success { id: Id },
    InvalidName { reason: String },
    NameAlreadyUsed,
}

// Create a Cmd enum for all commands and a Rpl enum for all replies, as well as
// TryFrom impls for the individual command and reply structs.
packets! {
    Hello(HelloCmd, HelloRpl),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Packet {
    Cmd {
        id: u64,
        #[serde(flatten)]
        cmd: Cmd,
    },
    Rpl {
        id: u64,
        #[serde(flatten)]
        rpl: Rpl,
    },
}
