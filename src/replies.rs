use std::collections::HashMap;
use std::hash::Hash;
use std::result;
use std::time::Duration;

use tokio::sync::oneshot::{self, Receiver, Sender};
use tokio::time;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("timed out")]
    TimedOut,
    #[error("canceled")]
    Canceled,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub struct PendingReply<R> {
    timeout: Duration,
    result: Receiver<R>,
}

impl<R> PendingReply<R> {
    pub async fn get(self) -> Result<R> {
        let result = time::timeout(self.timeout, self.result).await;
        match result {
            Err(_) => Err(Error::TimedOut),
            Ok(Err(_)) => Err(Error::Canceled),
            Ok(Ok(value)) => Ok(value),
        }
    }
}

#[derive(Debug)]
pub struct Replies<I, R> {
    timeout: Duration,
    pending: HashMap<I, Sender<R>>,
}

impl<I: Eq + Hash, R> Replies<I, R> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            pending: HashMap::new(),
        }
    }

    pub fn wait_for(&mut self, id: I) -> PendingReply<R> {
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, tx);
        PendingReply {
            timeout: self.timeout,
            result: rx,
        }
    }

    pub fn complete(&mut self, id: &I, result: R) {
        if let Some(tx) = self.pending.remove(id) {
            let _ = tx.send(result);
        }
    }

    pub fn purge(&mut self) {
        self.pending.retain(|_, tx| !tx.is_closed());
    }
}
