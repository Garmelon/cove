use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::traits::{Msg, MsgStore};

pub struct DummyMsg {
    id: usize,
    parent: Option<usize>,
    time: DateTime<Utc>,
    nick: String,
    content: String,
}

impl DummyMsg {
    fn new<T, S>(id: usize, time: T, nick: S, content: S) -> Self
    where
        T: Into<DateTime<Utc>>,
        S: Into<String>,
    {
        Self {
            id,
            parent: None,
            time: time.into(),
            nick: nick.into(),
            content: content.into(),
        }
    }

    fn parent(mut self, parent: usize) -> Self {
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
    fn new() -> Self {
        Self {
            msgs: HashMap::new(),
        }
    }

    fn msg(mut self, msg: DummyMsg) -> Self {
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
