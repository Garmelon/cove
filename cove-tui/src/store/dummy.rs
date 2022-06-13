use std::collections::HashMap;
use std::thread::Thread;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use super::{Msg, MsgStore, Path, Tree};

#[derive(Clone)]
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

    fn parent(&self) -> Option<Self::Id> {
        self.parent
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
    children: HashMap<usize, Vec<usize>>,
}

impl DummyStore {
    pub fn new() -> Self {
        Self {
            msgs: HashMap::new(),
            children: HashMap::new(),
        }
    }

    pub fn msg(mut self, msg: DummyMsg) -> Self {
        if let Some(parent) = msg.parent {
            self.children.entry(parent).or_default().push(msg.id());
        }
        self.msgs.insert(msg.id(), msg);
        self
    }

    fn tree(&self, id: usize, result: &mut Vec<DummyMsg>) {
        if let Some(msg) = self.msgs.get(&id) {
            result.push(msg.clone());
            if let Some(children) = self.children.get(&id) {
                for child in children {
                    self.tree(*child, result);
                }
            }
        }
    }
}

#[async_trait]
impl MsgStore<DummyMsg> for DummyStore {
    async fn path(&self, _room: &str, mut id: usize) -> Path<usize> {
        let mut segments = vec![id];
        while let Some(parent) = self.msgs.get(&id).and_then(|msg| msg.parent) {
            segments.push(parent);
            id = parent;
        }
        segments.reverse();
        Path::new(segments)
    }

    async fn thread(&self, _room: &str, root: usize) -> Tree<DummyMsg> {
        let mut msgs = vec![];

        Tree::new(root, msgs)
    }
}
