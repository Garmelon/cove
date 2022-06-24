use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use log::{error, info, warn};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task, time};
use tokio_tungstenite::tungstenite;

use crate::ui::UiEvent;
use crate::vault::EuphVault;

use super::api::{Data, Log, Snowflake};
use super::conn::{self, ConnRx, ConnTx, Status};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("room stopped")]
    Stopped,
}

#[derive(Debug)]
enum Event {
    Connected(ConnTx),
    Disconnected,
    Data(Data),
    Status(oneshot::Sender<Option<Status>>),
    RequestLogs,
}

#[derive(Debug)]
struct State {
    name: String,
    vault: EuphVault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    conn_tx: Option<ConnTx>,
    /// `None` before any `snapshot-event`, then either `Some(None)` or
    /// `Some(Some(id))`.
    last_msg_id: Option<Option<Snowflake>>,
    requesting_logs: Arc<Mutex<bool>>,
}

impl State {
    async fn run(
        mut self,
        canary: oneshot::Receiver<Infallible>,
        event_tx: mpsc::UnboundedSender<Event>,
        mut event_rx: mpsc::UnboundedReceiver<Event>,
    ) {
        let name = self.name.clone();
        let result = select! {
            _ = canary => Ok(()),
            _ = Self::reconnect(&name, &event_tx) => Ok(()),
            _ = Self::regularly_request_logs(&event_tx) => Ok(()),
            e = self.handle_events(&mut event_rx) => e,
        };

        if let Err(e) = result {
            error!("e&{name}: {}", e);
        }
    }

    async fn reconnect(name: &str, event_tx: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
        loop {
            info!("e&{}: connecting", name);
            let (conn_tx, mut conn_rx) = match Self::connect(name).await? {
                Some(conn) => conn,
                None => continue,
            };
            info!("e&{}: connected", name);
            event_tx.send(Event::Connected(conn_tx))?;

            while let Ok(data) = conn_rx.recv().await {
                event_tx.send(Event::Data(data))?;
            }

            info!("e&{}: disconnected", name);
            event_tx.send(Event::Disconnected)?;
            time::sleep(Duration::from_secs(5)).await; // TODO Make configurable
        }
    }

    async fn connect(name: &str) -> anyhow::Result<Option<(ConnTx, ConnRx)>> {
        // TODO Cookies
        let url = format!("wss://euphoria.io/room/{name}/ws");
        match tokio_tungstenite::connect_async(&url).await {
            Ok((ws, _)) => Ok(Some(conn::wrap(ws))),
            Err(tungstenite::Error::Http(resp)) if resp.status().is_client_error() => {
                bail!("room {name} doesn't exist");
            }
            Err(_) => Ok(None),
        }
    }

    async fn regularly_request_logs(event_tx: &mpsc::UnboundedSender<Event>) {
        loop {
            time::sleep(Duration::from_secs(10)).await; // TODO Make configurable
            let _ = event_tx.send(Event::RequestLogs);
        }
    }

    async fn handle_events(
        &mut self,
        event_rx: &mut mpsc::UnboundedReceiver<Event>,
    ) -> anyhow::Result<()> {
        while let Some(event) = event_rx.recv().await {
            match event {
                Event::Connected(conn_tx) => self.conn_tx = Some(conn_tx),
                Event::Disconnected => {
                    self.conn_tx = None;
                    self.last_msg_id = None;
                }
                Event::Data(data) => self.on_data(data).await?,
                Event::Status(reply_tx) => self.on_status(reply_tx).await,
                Event::RequestLogs => self.on_request_logs(),
            }
        }
        Ok(())
    }

    async fn on_data(&mut self, data: Data) -> anyhow::Result<()> {
        match data {
            Data::BounceEvent(_) => {
                error!("e&{}: auth not implemented", self.name);
                bail!("auth not implemented");
            }
            Data::DisconnectEvent(d) => {
                warn!("e&{}: disconnected for reason {:?}", self.name, d.reason);
            }
            Data::HelloEvent(_) => {}
            Data::JoinEvent(d) => {
                info!("e&{}: {:?} joined", self.name, d.0.name);
            }
            Data::LoginEvent(_) => {}
            Data::LogoutEvent(_) => {}
            Data::NetworkEvent(d) => {
                info!("e&{}: network event ({})", self.name, d.r#type);
            }
            Data::NickEvent(d) => {
                info!("e&{}: {:?} renamed to {:?}", self.name, d.from, d.to);
            }
            Data::EditMessageEvent(_) => {
                info!("e&{}: a message was edited", self.name);
            }
            Data::PartEvent(d) => {
                info!("e&{}: {:?} left", self.name, d.0.name);
            }
            Data::PingEvent(_) => {}
            Data::PmInitiateEvent(d) => {
                info!(
                    "e&{}: {:?} initiated a pm from &{}",
                    self.name, d.from_nick, d.from_room
                );
            }
            Data::SendEvent(d) => {
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    let id = d.0.id;
                    self.vault.add_message(d.0, *last_msg_id);
                    *last_msg_id = Some(id);
                    let _ = self.ui_event_tx.send(UiEvent::Redraw);
                } else {
                    bail!("send event before snapshot event");
                }
            }
            Data::SnapshotEvent(d) => {
                info!("e&{}: successfully joined", self.name);
                self.last_msg_id = Some(d.log.last().map(|m| m.id));
                self.vault.add_messages(d.log, None);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
            }
            Data::LogReply(d) => {
                self.vault.add_messages(d.log, d.before);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
            }
            Data::SendReply(d) => {
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    let id = d.0.id;
                    self.vault.add_message(d.0, *last_msg_id);
                    *last_msg_id = Some(id);
                    let _ = self.ui_event_tx.send(UiEvent::Redraw);
                } else {
                    bail!("send reply before snapshot event");
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn on_status(&self, reply_tx: oneshot::Sender<Option<Status>>) {
        let status = if let Some(conn_tx) = &self.conn_tx {
            conn_tx.status().await.ok()
        } else {
            None
        };

        let _ = reply_tx.send(status);
    }

    fn on_request_logs(&self) {
        if let Some(conn_tx) = &self.conn_tx {
            // Check whether logs are already being requested
            let mut guard = self.requesting_logs.lock();
            if *guard {
                return;
            } else {
                *guard = true;
            }
            drop(guard);

            // No logs are being requested and we've reserved our spot, so let's
            // request some logs!
            let vault = self.vault.clone();
            let conn_tx = conn_tx.clone();
            let requesting_logs = self.requesting_logs.clone();
            task::spawn(async move {
                let result = Self::request_logs(vault, conn_tx).await;
                *requesting_logs.lock() = false;
                result
            });
        }
    }

    async fn request_logs(vault: EuphVault, conn_tx: ConnTx) -> anyhow::Result<()> {
        let before = match vault.last_span().await {
            Some((None, _)) => return Ok(()), // Already at top of room history
            Some((Some(before), _)) => Some(before),
            None => None,
        };

        let _ = conn_tx.send(Log { n: 1000, before }).await?;
        // The code handling incoming events and replies also handles
        // `LogReply`s, so we don't need to do anything special here.

        Ok(())
    }
}

#[derive(Debug)]
pub struct Room {
    #[allow(dead_code)]
    canary: oneshot::Sender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl Room {
    pub fn new(
        name: String,
        vault: EuphVault,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        let (canary_tx, canary_rx) = oneshot::channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let state = State {
            name,
            vault,
            ui_event_tx,
            conn_tx: None,
            last_msg_id: None,
            requesting_logs: Arc::new(Mutex::new(false)),
        };

        task::spawn(state.run(canary_rx, event_tx.clone(), event_rx));

        Self {
            canary: canary_tx,
            event_tx,
        }
    }

    pub async fn status(&self) -> Result<Option<Status>, Error> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Status(tx))
            .map_err(|_| Error::Stopped)?;
        rx.await.map_err(|_| Error::Stopped)
    }

    pub fn request_logs(&self) -> Result<(), Error> {
        self.event_tx
            .send(Event::RequestLogs)
            .map_err(|_| Error::Stopped)
    }
}
