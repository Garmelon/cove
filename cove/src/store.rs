use std::{collections::HashMap, fmt::Debug, hash::Hash, vec};

use async_trait::async_trait;

pub trait Msg {
    type Id: Clone + Debug + Hash + Eq + Ord;
    fn id(&self) -> Self::Id;
    fn parent(&self) -> Option<Self::Id>;
    fn seen(&self) -> bool;

    fn nick_emoji(&self) -> Option<String> {
        None
    }

    fn last_possible_id() -> Self::Id;
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Path<I>(Vec<I>);

impl<I> Path<I> {
    pub fn new(segments: Vec<I>) -> Self {
        assert!(!segments.is_empty(), "segments must not be empty");
        Self(segments)
    }

    pub fn parent_segments(&self) -> impl Iterator<Item = &I> {
        self.0.iter().take(self.0.len() - 1)
    }

    pub fn first(&self) -> &I {
        self.0.first().expect("path is empty")
    }

    pub fn into_first(self) -> I {
        self.0.into_iter().next().expect("path is empty")
    }
}

impl<I> IntoIterator for Path<I> {
    type Item = I;
    type IntoIter = vec::IntoIter<I>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
            children.entry(msg.id()).or_default();
            if let Some(parent) = msg.parent() {
                children.entry(parent).or_default().push(msg.id());
            }
        }

        for list in children.values_mut() {
            list.sort_unstable();
        }

        Self {
            root,
            msgs,
            children,
        }
    }

    pub fn len(&self) -> usize {
        self.msgs.len()
    }

    pub fn root(&self) -> &M::Id {
        &self.root
    }

    pub fn msg(&self, id: &M::Id) -> Option<&M> {
        self.msgs.get(id)
    }

    pub fn parent(&self, id: &M::Id) -> Option<M::Id> {
        self.msg(id).and_then(|m| m.parent())
    }

    pub fn children(&self, id: &M::Id) -> Option<&[M::Id]> {
        self.children.get(id).map(|c| c as &[M::Id])
    }

    pub fn subtree_size(&self, id: &M::Id) -> usize {
        let children = self.children(id).unwrap_or_default();
        let mut result = children.len();
        for child in children {
            result += self.subtree_size(child);
        }
        result
    }

    pub fn siblings(&self, id: &M::Id) -> Option<&[M::Id]> {
        if let Some(parent) = self.parent(id) {
            self.children(&parent)
        } else {
            None
        }
    }

    pub fn prev_sibling(&self, id: &M::Id) -> Option<M::Id> {
        let siblings = self.siblings(id)?;
        siblings
            .iter()
            .zip(siblings.iter().skip(1))
            .find(|(_, s)| *s == id)
            .map(|(s, _)| s.clone())
    }

    pub fn next_sibling(&self, id: &M::Id) -> Option<M::Id> {
        let siblings = self.siblings(id)?;
        siblings
            .iter()
            .zip(siblings.iter().skip(1))
            .find(|(s, _)| *s == id)
            .map(|(_, s)| s.clone())
    }
}

#[allow(dead_code)]
#[async_trait]
pub trait MsgStore<M: Msg> {
    type Error;
    async fn path(&self, id: &M::Id) -> Result<Path<M::Id>, Self::Error>;
    async fn msg(&self, id: &M::Id) -> Result<Option<M>, Self::Error>;
    async fn tree(&self, root_id: &M::Id) -> Result<Tree<M>, Self::Error>;
    async fn first_root_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn last_root_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn prev_root_id(&self, root_id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn next_root_id(&self, root_id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn oldest_msg_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn newest_msg_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn older_msg_id(&self, id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn newer_msg_id(&self, id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn oldest_unseen_msg_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn newest_unseen_msg_id(&self) -> Result<Option<M::Id>, Self::Error>;
    async fn older_unseen_msg_id(&self, id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn newer_unseen_msg_id(&self, id: &M::Id) -> Result<Option<M::Id>, Self::Error>;
    async fn unseen_msgs_count(&self) -> Result<usize, Self::Error>;
    async fn set_seen(&self, id: &M::Id, seen: bool) -> Result<(), Self::Error>;
    async fn set_older_seen(&self, id: &M::Id, seen: bool) -> Result<(), Self::Error>;
}
