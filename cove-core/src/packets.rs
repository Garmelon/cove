use serde::{Deserialize, Serialize};

use crate::macros::packets;
use crate::{Message, MessageId, User};

#[derive(Debug, Deserialize, Serialize)]
pub struct HelloCmd {
    pub nick: String,
    pub identity: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum HelloRpl {
    Success {
        you: User,
        others: Vec<User>,
        last_message: MessageId,
    },
    NickTooLong,
    IdentityTooLong,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NickCmd {
    pub nick: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum NickRpl {
    Success,
    NickTooLong,
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
    NickTooLong,
    ContentTooLong,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WhoCmd {}

#[derive(Debug, Deserialize, Serialize)]
pub struct WhoRpl {
    pub you: User,
    pub others: Vec<User>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JoinNtf {
    pub user: User,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NickNtf {
    pub user: User,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PartNtf {
    pub user: User,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SendNtf {
    pub message: Message,
}

// Create a Cmd enum for all commands, a Rpl enum for all replies and a Ntf enum
// for all notifications, as well as TryFrom impls for the individual structs.
packets! {
    cmd Hello(HelloCmd, HelloRpl),
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
