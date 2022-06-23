use std::sync::Arc;
use std::vec;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::{Level, Log};
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::store::{Msg, MsgStore, Path, Tree};

#[derive(Debug, Clone)]
pub struct LogMsg {
    id: usize,
    time: DateTime<Utc>,
    level: Level,
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
        format!("{}", self.level)
    }

    fn content(&self) -> String {
        self.content.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Logger {
    event_tx: mpsc::UnboundedSender<()>,
    messages: Arc<Mutex<Vec<LogMsg>>>,
}

#[async_trait]
impl MsgStore<LogMsg> for Logger {
    async fn path(&self, id: &usize) -> Path<usize> {
        Path::new(vec![*id])
    }

    async fn tree(&self, root: &usize) -> Tree<LogMsg> {
        let msgs = self
            .messages
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
        let len = self.messages.lock().len();
        tree.checked_add(1).filter(|t| *t < len)
    }

    async fn first_tree(&self) -> Option<usize> {
        let empty = self.messages.lock().is_empty();
        Some(0).filter(|_| !empty)
    }

    async fn last_tree(&self) -> Option<usize> {
        self.messages.lock().len().checked_sub(1)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut guard = self.messages.lock();
        let msg = LogMsg {
            id: guard.len(),
            time: Utc::now(),
            level: record.level(),
            content: format!("<{}> {}", record.target(), record.args()),
        };
        guard.push(msg);

        let _ = self.event_tx.send(());
    }

    fn flush(&self) {}
}

impl Logger {
    pub fn init(level: Level) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let logger = Self {
            event_tx,
            messages: Arc::new(Mutex::new(Vec::new())),
        };

        log::set_boxed_logger(Box::new(logger.clone())).expect("logger already set");
        log::set_max_level(level.to_level_filter());

        (logger, event_rx)
    }
}
