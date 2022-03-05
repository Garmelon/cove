use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{self, Sender};
use tokio::sync::{Mutex, MutexGuard};

use crate::config::Config;
use crate::never::Never;

use super::super::Event;
use super::conn::{self, CoveConn, CoveConnMt};

struct ConnConfig {
    url: String,
    room: String,
    timeout: Duration,
    ev_tx: UnboundedSender<conn::Event>,
}

impl ConnConfig {
    async fn new_conn(&self) -> (CoveConn, CoveConnMt) {
        conn::new(
            self.url.clone(),
            self.room.clone(),
            self.timeout,
            self.ev_tx.clone(),
        )
        .await
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
    pub async fn new<E>(
        config: &'static Config,
        outer_ev_tx: UnboundedSender<E>,
        name: String,
    ) -> Self
    where
        E: Send + 'static,
        Event: Into<E>,
    {
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        let (tx, rx) = oneshot::channel();

        let conf = ConnConfig {
            ev_tx,
            url: config.cove_url.to_string(),
            room: name.clone(),
            timeout: config.timeout,
        };
        let (conn, mt) = conf.new_conn().await;

        let room = Self {
            name: name.clone(),
            conn: Arc::new(Mutex::new(conn)),
            dead_mans_switch: tx,
        };

        let conn_clone = room.conn.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = rx => {} // Watch dead man's switch
                _ = Self::shovel_events(ev_rx, outer_ev_tx, name) => {}
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
        mut ev_rx: UnboundedReceiver<conn::Event>,
        ev_tx: UnboundedSender<E>,
        name: String,
    ) where
        Event: Into<E>,
    {
        while let Some(event) = ev_rx.recv().await {
            let event = Event::Cove(name.clone(), event);
            if ev_tx.send(event.into()).is_err() {
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
            let (new_conn, new_mt) = conf.new_conn().await;
            *conn.lock().await = new_conn;
            mt = new_mt;
        }
    }
}
