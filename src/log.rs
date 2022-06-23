use std::sync::Arc;
use std::vec;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use crate::store::{Msg, MsgStore, Path, Tree};

#[derive(Debug, Clone)]
pub struct LogMsg {
    id: usize,
    time: DateTime<Utc>,
    topic: String,
    content: String,
}

impl Msg for LogMsg {
    type Id = usize;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn parent(&self) -> Option<Self::Id> {
        None
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    fn nick(&self) -> String {
        self.topic.clone()
    }

    fn content(&self) -> String {
        self.content.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Log {
    entries: Arc<Mutex<Vec<LogMsg>>>,
}

#[async_trait]
impl MsgStore<LogMsg> for Log {
    async fn path(&self, id: &usize) -> Path<usize> {
        Path::new(vec![*id])
    }

    async fn tree(&self, root: &usize) -> Tree<LogMsg> {
        let msgs = self
            .entries
            .lock()
            .get(*root)
            .map(|msg| vec![msg.clone()])
            .unwrap_or_default();
        Tree::new(*root, msgs)
    }

    async fn prev_tree(&self, tree: &usize) -> Option<usize> {
        tree.checked_sub(1)
    }

    async fn next_tree(&self, tree: &usize) -> Option<usize> {
        let len = self.entries.lock().len();
        tree.checked_add(1).filter(|t| *t < len)
    }

    async fn first_tree(&self) -> Option<usize> {
        let empty = self.entries.lock().is_empty();
        Some(0).filter(|_| !empty)
    }

    async fn last_tree(&self) -> Option<usize> {
        self.entries.lock().len().checked_sub(1)
    }
}

impl Log {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log<S1: ToString, S2: ToString>(&self, topic: S1, content: S2) {
        let mut guard = self.entries.lock();
        let msg = LogMsg {
            id: guard.len(),
            time: Utc::now(),
            topic: topic.to_string(),
            content: content.to_string(),
        };
        guard.push(msg);
    }
}
