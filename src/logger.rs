use std::sync::Arc;
use std::vec;

use async_trait::async_trait;
use crossterm::style::{ContentStyle, Stylize};
use log::{Level, Log};
use parking_lot::Mutex;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use toss::styled::Styled;

use crate::store::{Msg, MsgStore, Path, Tree};
use crate::ui::ChatMsg;

#[derive(Debug, Clone)]
pub struct LogMsg {
    id: usize,
    time: OffsetDateTime,
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

    fn seen(&self) -> bool {
        true
    }

    fn last_possible_id() -> Self::Id {
        Self::Id::MAX
    }
}

impl ChatMsg for LogMsg {
    fn time(&self) -> OffsetDateTime {
        self.time
    }

    fn styled(&self) -> (Styled, Styled) {
        let nick_style = match self.level {
            Level::Error => ContentStyle::default().bold().red(),
            Level::Warn => ContentStyle::default().bold().yellow(),
            Level::Info => ContentStyle::default().bold().green(),
            Level::Debug => ContentStyle::default().bold().blue(),
            Level::Trace => ContentStyle::default().bold().magenta(),
        };
        let nick = Styled::new(format!("{}", self.level), nick_style);
        let content = Styled::new_plain(&self.content);
        (nick, content)
    }

    fn edit(_nick: &str, _content: &str) -> (Styled, Styled) {
        panic!("log is not editable")
    }

    fn pseudo(_nick: &str, _content: &str) -> (Styled, Styled) {
        panic!("log is not editable")
    }
}

/// Prints all error messages when dropped.
pub struct LoggerGuard {
    messages: Arc<Mutex<Vec<LogMsg>>>,
}

impl Drop for LoggerGuard {
    fn drop(&mut self) {
        let guard = self.messages.lock();
        let mut error_encountered = false;
        for msg in &*guard {
            if msg.level == Level::Error {
                if !error_encountered {
                    eprintln!();
                    eprintln!("The following errors occurred while cove was running:");
                }
                error_encountered = true;
                eprintln!("{}", msg.content);
            }
        }
        if error_encountered {
            eprintln!();
        }
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

    async fn msg(&self, id: &usize) -> Option<LogMsg> {
        self.messages.lock().get(*id).cloned()
    }

    async fn tree(&self, root_id: &usize) -> Tree<LogMsg> {
        let msgs = self
            .messages
            .lock()
            .get(*root_id)
            .map(|msg| vec![msg.clone()])
            .unwrap_or_default();
        Tree::new(*root_id, msgs)
    }

    async fn first_root_id(&self) -> Option<usize> {
        let empty = self.messages.lock().is_empty();
        Some(0).filter(|_| !empty)
    }

    async fn last_root_id(&self) -> Option<usize> {
        self.messages.lock().len().checked_sub(1)
    }

    async fn prev_root_id(&self, root_id: &usize) -> Option<usize> {
        root_id.checked_sub(1)
    }

    async fn next_root_id(&self, root_id: &usize) -> Option<usize> {
        let len = self.messages.lock().len();
        root_id.checked_add(1).filter(|t| *t < len)
    }

    async fn oldest_msg_id(&self) -> Option<usize> {
        self.first_root_id().await
    }

    async fn newest_msg_id(&self) -> Option<usize> {
        self.last_root_id().await
    }

    async fn older_msg_id(&self, id: &usize) -> Option<usize> {
        self.prev_root_id(id).await
    }

    async fn newer_msg_id(&self, id: &usize) -> Option<usize> {
        self.next_root_id(id).await
    }

    async fn oldest_unseen_msg_id(&self) -> Option<usize> {
        None
    }

    async fn newest_unseen_msg_id(&self) -> Option<usize> {
        None
    }

    async fn older_unseen_msg_id(&self, _id: &usize) -> Option<usize> {
        None
    }

    async fn newer_unseen_msg_id(&self, _id: &usize) -> Option<usize> {
        None
    }

    async fn unseen_msgs_count(&self) -> usize {
        0
    }

    async fn set_seen(&self, _id: &usize, _seen: bool) {}

    async fn set_older_seen(&self, _id: &usize, _seen: bool) {}
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.level() <= Level::Info {
            return true;
        }

        let target = metadata.target();
        if target.starts_with("cove") || target.starts_with("euphoxide::bot") {
            return true;
        }

        false
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut guard = self.messages.lock();
        let msg = LogMsg {
            id: guard.len(),
            time: OffsetDateTime::now_utc(),
            level: record.level(),
            content: format!("<{}> {}", record.target(), record.args()),
        };
        guard.push(msg);

        let _ = self.event_tx.send(());
    }

    fn flush(&self) {}
}

impl Logger {
    pub fn init(level: Level) -> (Self, LoggerGuard, mpsc::UnboundedReceiver<()>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let logger = Self {
            event_tx,
            messages: Arc::new(Mutex::new(Vec::new())),
        };
        let guard = LoggerGuard {
            messages: logger.messages.clone(),
        };

        log::set_boxed_logger(Box::new(logger.clone())).expect("logger already set");
        log::set_max_level(level.to_level_filter());

        (logger, guard, event_rx)
    }
}
