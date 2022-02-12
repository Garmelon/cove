use serde::{Deserialize, Serialize};

use crate::{Identity, MessageId};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub time: u128,
    pub pred: MessageId,
    pub parent: Option<MessageId>,
    pub identity: Identity,
    pub nick: String,
    pub content: String,
}

impl Message {
    pub fn id(&self) -> MessageId {
        let time = self.time;
        let pred = self.pred;
        let parent = match self.parent {
            Some(id) => format!("{id}"),
            None => "none".to_string(),
        };
        let identity = self.identity;
        let nick = MessageId::of(&self.nick);
        let content = MessageId::of(&self.content);
        let str = format!("message {time} {pred} {parent} {identity} {nick} {content}");
        MessageId::of(&str)
    }
}
