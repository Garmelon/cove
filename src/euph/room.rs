use std::convert::Infallible;
use std::time::Duration;

use anyhow::bail;
use log::{error, info, warn};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task, time};
use tokio_tungstenite::tungstenite;

use super::api::Data;
use super::conn::{self, ConnRx, ConnTx, Status, WsStream};

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
    conn_tx: Option<ConnTx>,
}

impl State {
    async fn run(
        name: String,
        canary: oneshot::Receiver<Infallible>,
        event_tx: mpsc::UnboundedSender<Event>,
        mut event_rx: mpsc::UnboundedReceiver<Event>,
    ) {
        let mut state = Self {
            name: name.clone(),
            conn_tx: None,
        };

        let result = select! {
            _ = canary => Ok(()),
            _ = Self::reconnect(&name, &event_tx) => Ok(()),
            e = state.handle_events(&mut event_rx) => e,
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
                Event::Disconnected => self.conn_tx = None,
                Event::Data(data) => self.on_data(data).await?,
                Event::Status(reply_tx) => self.on_status(reply_tx).await,
            }
        }
        Ok(())
    }

    async fn on_data(&self, data: Data) -> anyhow::Result<()> {
        match data {
            Data::BounceEvent(_) => {
                error!("e&{}: auth not implemented", self.name);
                bail!("auth not implemented");
            }
            Data::DisconnectEvent(e) => {
                warn!("e&{}: disconnected for reason {:?}", self.name, e.reason);
            }
            Data::HelloEvent(_) => {}
            Data::JoinEvent(e) => {
                info!("e&{}: {:?} joined", self.name, e.0.name);
            }
            Data::LoginEvent(_) => {}
            Data::LogoutEvent(_) => {}
            Data::NetworkEvent(e) => {
                info!("e&{}: network event ({})", self.name, e.r#type);
            }
            Data::NickEvent(e) => {
                info!("e&{}: {:?} renamed to {:?}", self.name, e.from, e.to);
            }
            Data::EditMessageEvent(_) => {
                info!("e&{}: a message was edited", self.name);
            }
            Data::PartEvent(e) => {
                info!("e&{}: {:?} left", self.name, e.0.name);
            }
            Data::PingEvent(_) => {}
            Data::PmInitiateEvent(e) => {
                info!(
                    "e&{}: {:?} initiated a pm from &{}",
                    self.name, e.from_nick, e.from_room
                );
            }
            Data::SendEvent(_) => {}
            Data::SnapshotEvent(e) => {
                info!("e&{}: successfully joined", self.name);
                if let Some(nick) = e.nick {
                    info!("e&{}: using nick {nick:?}", self.name);
                } else {
                    info!("e&{}: no nick set", self.name);
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
}

#[derive(Debug)]
pub struct Room {
    canary: oneshot::Sender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl Room {
    pub fn new(name: String) -> Self {
        let (canary_tx, canary_rx) = oneshot::channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        task::spawn(State::run(name, canary_rx, event_tx.clone(), event_rx));

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
