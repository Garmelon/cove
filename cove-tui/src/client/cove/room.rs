use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{self, Sender};
use tokio::sync::{Mutex, MutexGuard};

use crate::config::Config;
use crate::never::Never;

use super::conn::{self, CoveConn, CoveConnMt, Event};

struct ConnConfig {
    url: String,
    room: String,
    timeout: Duration,
    ev_tx: UnboundedSender<conn::Event>,
}

impl ConnConfig {
    fn new_conn(&self) -> (CoveConn, CoveConnMt) {
        conn::new(
            self.url.clone(),
            self.room.clone(),
            self.timeout,
            self.ev_tx.clone(),
        )
    }
}

pub struct CoveRoom {
    name: String,
    conn: Arc<Mutex<CoveConn>>,
    /// Once this is dropped, all other room-related tasks, connections and
    /// values are cleaned up. It is never used to send actual values.
    #[allow(dead_code)]
    dead_mans_switch: Sender<Never>,
}

impl CoveRoom {
    /// This method uses [`tokio::spawn`] and must thus be called in the context
    /// of a tokio runtime.
    pub fn new<E, F>(
        config: &'static Config,
        name: String,
        event_sender: UnboundedSender<E>,
        convert_event: F,
    ) -> Self
    where
        E: Send + 'static,
        F: Fn(&str, Event) -> E + Send + 'static,
    {
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        let (tx, rx) = oneshot::channel();

        let conf = ConnConfig {
            ev_tx,
            url: config.cove_url.to_string(),
            room: name.clone(),
            timeout: config.timeout,
        };
        let (conn, mt) = conf.new_conn();

        let room = Self {
            name: name.clone(),
            conn: Arc::new(Mutex::new(conn)),
            dead_mans_switch: tx,
        };

        let conn_clone = room.conn.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = rx => {} // Watch dead man's switch
                _ = Self::shovel_events(name, ev_rx, event_sender, convert_event) => {}
                _ = Self::run(conn_clone, mt, conf) => {}
            }
        });

        room
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    // TODO Disallow modification via this MutexGuard
    pub async fn conn(&self) -> MutexGuard<'_, CoveConn> {
        self.conn.lock().await
    }

    async fn shovel_events<E>(
        name: String,
        mut ev_rx: UnboundedReceiver<conn::Event>,
        ev_tx: UnboundedSender<E>,
        convert_event: impl Fn(&str, Event) -> E,
    ) {
        while let Some(event) = ev_rx.recv().await {
            let event = convert_event(&name, event);
            if ev_tx.send(event).is_err() {
                break;
            }
        }
    }

    /// Background task to connect to a room and stay connected.
    async fn run(conn: Arc<Mutex<CoveConn>>, mut mt: CoveConnMt, conf: ConnConfig) {
        // We have successfully connected to the url before. Errors while
        // connecting are probably not our fault and we should try again later.
        let mut url_exists = false;

        loop {
            match mt.run().await {
                Err(conn::Error::CouldNotConnect(_)) if url_exists => {
                    // TODO Exponential backoff?
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
                Err(conn::Error::CouldNotConnect(_)) => return,
                Err(conn::Error::InvalidRoom(_)) => return,
                Err(conn::Error::InvalidIdentity(_)) => return,
                _ => {}
            }

            url_exists = true;

            // TODO Clean up with restructuring assignments later?
            let (new_conn, new_mt) = conf.new_conn();
            *conn.lock().await = new_conn;
            mt = new_mt;
        }
    }
}
