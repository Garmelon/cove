use serde::{Deserialize, Serialize};

use crate::Id;

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub pred: Option<Id>,
    pub parent: Option<Id>,
    pub identity: Id,
    pub nick: String,
    pub content: String,
}

impl Message {
    pub fn id(&self) -> Id {
        let pred = match self.pred {
            Some(id) => format!("{id}"),
            None => "none".to_string(),
        };
        let parent = match self.parent {
            Some(id) => format!("{id}"),
            None => "none".to_string(),
        };
        let identity = self.identity;
        let nick = Id::of(&self.nick);
        let content = Id::of(&self.content);
        let str = format!("message {pred} {parent} {identity} {nick} {content}");
        Id::of(&str)
    }
}
