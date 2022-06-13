pub mod dummy;

use std::collections::HashMap;
use std::hash::Hash;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub trait Msg {
    type Id: Hash + Eq;
    fn id(&self) -> Self::Id;
    fn parent(&self) -> Option<Self::Id>;

    fn time(&self) -> DateTime<Utc>;
    fn nick(&self) -> String;
    fn content(&self) -> String;
}

pub struct Path<I>(Vec<I>);

impl<I> Path<I> {
    pub fn new(segments: Vec<I>) -> Self {
        assert!(!segments.is_empty(), "segments must not be empty");
        Self(segments)
    }

    pub fn segments(&self) -> &[I] {
        &self.0
    }

    pub fn first(&self) -> &I {
        self.0.first().expect("path is not empty")
    }

    pub fn first_mut(&mut self) -> &mut I {
        self.0.first_mut().expect("path is not empty")
    }

    pub fn last(&self) -> &I {
        self.0.last().expect("path is not empty")
    }

    pub fn last_mut(&mut self) -> &mut I {
        self.0.last_mut().expect("path is not empty")
    }
}

pub struct Tree<M: Msg> {
    root: M::Id,
    msgs: HashMap<M::Id, M>,
    children: HashMap<M::Id, Vec<M::Id>>,
}

impl<M: Msg> Tree<M> {
    pub fn new(root: M::Id, msgs: Vec<M>) -> Self {
        let msgs: HashMap<M::Id, M> = msgs.into_iter().map(|m| (m.id(), m)).collect();

        let mut children: HashMap<M::Id, Vec<M::Id>> = HashMap::new();
        for msg in msgs.values() {
            if let Some(parent) = msg.parent() {
                children.entry(parent).or_default().push(msg.id());
            }
        }

        Self {
            root,
            msgs,
            children,
        }
    }

    pub fn root(&self) -> &M::Id {
        &self.root
    }

    pub fn msg(&self, id: &M::Id) -> Option<&M> {
        self.msgs.get(id)
    }

    pub fn children(&self, id: &M::Id) -> Option<&[M::Id]> {
        self.children.get(id).map(|c| c as &[M::Id])
    }
}

#[async_trait]
pub trait MsgStore<M: Msg> {
    async fn path(&self, room: &str, id: M::Id) -> Path<M::Id>;
    async fn thread(&self, room: &str, root: M::Id) -> Tree<M>;
}
