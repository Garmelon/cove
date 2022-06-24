use std::convert::Infallible;
use std::time::Duration;

use anyhow::bail;
use log::{error, info, warn};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task, time};
use tokio_tungstenite::tungstenite;

use crate::ui::UiEvent;
use crate::vault::EuphVault;

use super::api::{Data, Snowflake};
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
}

#[derive(Debug)]
struct State {
    name: String,
    vault: EuphVault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    conn_tx: Option<ConnTx>,
    last_msg_id: Option<Snowflake>,
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
                let id = d.0.id;
                self.vault.add_message(d.0, self.last_msg_id);
                self.last_msg_id = Some(id);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
            }
            Data::SnapshotEvent(d) => {
                info!("e&{}: successfully joined", self.name);
                self.last_msg_id = d.log.last().map(|m| m.id);
                self.vault.add_messages(d.log, None);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
            }
            Data::LogReply(d) => {
                self.vault.add_messages(d.log, d.before);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
            }
            Data::SendReply(d) => {
                let id = d.0.id;
                self.vault.add_message(d.0, self.last_msg_id);
                self.last_msg_id = Some(id);
                let _ = self.ui_event_tx.send(UiEvent::Redraw);
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
}
