use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use crate::traits::{Msg, MsgStore};

pub struct DummyMsg {
    id: usize,
    parent: Option<usize>,
    time: DateTime<Utc>,
    nick: String,
    content: String,
}

impl DummyMsg {
    pub fn new<S>(id: usize, nick: S, content: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            id,
            parent: None,
            time: Utc.timestamp(0, 0),
            nick: nick.into(),
            content: content.into(),
        }
    }

    pub fn parent(mut self, parent: usize) -> Self {
        self.parent = Some(parent);
        self
    }
}

impl Msg for DummyMsg {
    type Id = usize;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    fn nick(&self) -> String {
        self.nick.clone()
    }

    fn content(&self) -> String {
        self.content.clone()
    }
}

pub struct DummyStore {
    msgs: HashMap<usize, DummyMsg>,
}

impl DummyStore {
    pub fn new() -> Self {
        Self {
            msgs: HashMap::new(),
        }
    }

    pub fn msg(mut self, msg: DummyMsg) -> Self {
        self.msgs.insert(msg.id(), msg);
        self
    }
}

#[async_trait]
impl MsgStore<DummyMsg> for DummyStore {
    async fn path(&self, _room: &str, mut id: usize) -> Vec<usize> {
        let mut path = vec![id];

        while let Some(parent) = self.msgs.get(&id).and_then(|msg| msg.parent) {
            path.push(parent);
            id = parent;
        }

        path.reverse();
        path
    }
}
