use serde::{Deserialize, Serialize};

use crate::macros::packets;
use crate::{Message, MessageId, Session};

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomCmd {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RoomRpl {
    Success,
    InvalidRoom { reason: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IdentifyCmd {
    pub nick: String,
    pub identity: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum IdentifyRpl {
    Success {
        you: Session,
        others: Vec<Session>,
        last_message: MessageId,
    },
    InvalidNick {
        reason: String,
    },
    InvalidIdentity {
        reason: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NickCmd {
    pub nick: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum NickRpl {
    Success { you: Session },
    InvalidNick { reason: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SendCmd {
    pub parent: Option<MessageId>,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SendRpl {
    Success { message: Message },
    InvalidContent { reason: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WhoCmd {}

#[derive(Debug, Deserialize, Serialize)]
pub struct WhoRpl {
    pub you: Session,
    pub others: Vec<Session>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JoinNtf {
    pub who: Session,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NickNtf {
    pub who: Session,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PartNtf {
    pub who: Session,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SendNtf {
    pub message: Message,
}

// Create a Cmd enum for all commands, a Rpl enum for all replies and a Ntf enum
// for all notifications, as well as TryFrom impls for the individual structs.
packets! {
    cmd Room(RoomCmd, RoomRpl),
    cmd Identify(IdentifyCmd, IdentifyRpl),
    cmd Nick(NickCmd, NickRpl),
    cmd Send(SendCmd, SendRpl),
    cmd Who(WhoCmd, WhoRpl),
    ntf Join(JoinNtf),
    ntf Nick(NickNtf),
    ntf Part(PartNtf),
    ntf Send(SendNtf),
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
    Ntf {
        #[serde(flatten)]
        ntf: Ntf,
    },
}

impl Packet {
    pub fn cmd<C: Into<Cmd>>(id: u64, cmd: C) -> Self {
        Self::Cmd {
            id,
            cmd: cmd.into(),
        }
    }

    pub fn rpl<R: Into<Rpl>>(id: u64, rpl: R) -> Self {
        Self::Rpl {
            id,
            rpl: rpl.into(),
        }
    }

    pub fn ntf<N: Into<Ntf>>(ntf: N) -> Self {
        Self::Ntf { ntf: ntf.into() }
    }
}
