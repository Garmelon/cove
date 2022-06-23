use std::collections::{HashMap, HashSet};

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

    fn collect_tree(&self, id: usize, result: &mut Vec<DummyMsg>) {
        if let Some(msg) = self.msgs.get(&id) {
            result.push(msg.clone());
        }
        if let Some(children) = self.children.get(&id) {
            for child in children {
                self.collect_tree(*child, result);
            }
        }
    }

    fn trees(&self) -> Vec<usize> {
        let mut trees = HashSet::new();
        for m in self.msgs.values() {
            match m.parent() {
                Some(parent) if !self.msgs.contains_key(&parent) => {
                    trees.insert(parent);
                }
                Some(_) => {}
                None => {
                    trees.insert(m.id());
                }
            }
        }
        let mut trees: Vec<usize> = trees.into_iter().collect();
        trees.sort_unstable();
        trees
    }
}

#[async_trait]
impl MsgStore<DummyMsg> for DummyStore {
    async fn path(&self, id: &usize) -> Path<usize> {
        let mut id = *id;
        let mut segments = vec![id];
        while let Some(parent) = self.msgs.get(&id).and_then(|msg| msg.parent) {
            segments.push(parent);
            id = parent;
        }
        segments.reverse();
        Path::new(segments)
    }

    async fn tree(&self, root: &usize) -> Tree<DummyMsg> {
        let mut msgs = vec![];
        self.collect_tree(*root, &mut msgs);
        Tree::new(*root, msgs)
    }

    async fn prev_tree(&self, tree: &usize) -> Option<usize> {
        let trees = self.trees();
        trees
            .iter()
            .zip(trees.iter().skip(1))
            .find(|(_, t)| *t == tree)
            .map(|(t, _)| *t)
    }

    async fn next_tree(&self, tree: &usize) -> Option<usize> {
        let trees = self.trees();
        trees
            .iter()
            .zip(trees.iter().skip(1))
            .find(|(t, _)| *t == tree)
            .map(|(_, t)| *t)
    }

    async fn first_tree(&self) -> Option<usize> {
        self.trees().first().cloned()
    }

    async fn last_tree(&self) -> Option<usize> {
        self.trees().last().cloned()
    }
}
