use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use cookie::{Cookie, CookieJar};
use log::{error, info, warn};
use parking_lot::Mutex;
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task};
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue};

use crate::macros::ok_or_return;
use crate::ui::UiEvent;
use crate::vault::{EuphVault, Vault};

use super::api::{Data, Log, Nick, Send, Snowflake};
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
    Nick(String),
    Send(Option<Snowflake>, String),
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
        let vault = self.vault.clone();
        let name = self.name.clone();
        let result = select! {
            _ = canary => Ok(()),
            _ = Self::reconnect(&vault, &name, &event_tx) => Ok(()),
            _ = Self::regularly_request_logs(&event_tx) => Ok(()),
            e = self.handle_events(&mut event_rx) => e,
        };

        if let Err(e) = result {
            error!("e&{name}: {}", e);
        }
    }

    async fn reconnect(
        vault: &EuphVault,
        name: &str,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<()> {
        loop {
            info!("e&{}: connecting", name);
            if let Some((conn_tx, mut conn_rx)) = Self::connect(vault, name).await? {
                info!("e&{}: connected", name);
                event_tx.send(Event::Connected(conn_tx))?;

                while let Ok(data) = conn_rx.recv().await {
                    event_tx.send(Event::Data(data))?;
                }

                info!("e&{}: disconnected", name);
                event_tx.send(Event::Disconnected)?;
            } else {
                info!("e&{}: could not connect", name);
            }
            tokio::time::sleep(Duration::from_secs(5)).await; // TODO Make configurable
        }
    }

    async fn get_cookies(vault: &Vault) -> String {
        let cookie_jar = vault.euph_cookies().await;
        let cookies = cookie_jar
            .iter()
            .map(|c| format!("{}", c.stripped()))
            .collect::<Vec<_>>();
        cookies.join("; ")
    }

    async fn update_cookies(vault: &Vault, response: &Response) {
        let mut cookie_jar = CookieJar::new();

        for (name, value) in response.headers() {
            if name == header::SET_COOKIE {
                let value_str = ok_or_return!(value.to_str());
                let cookie = ok_or_return!(Cookie::from_str(value_str));
                cookie_jar.add(cookie);
            }
        }

        vault.set_euph_cookies(cookie_jar).await;
    }

    async fn connect(vault: &EuphVault, name: &str) -> anyhow::Result<Option<(ConnTx, ConnRx)>> {
        let uri = format!("wss://euphoria.io/room/{name}/ws?h=1");
        let mut request = uri.into_client_request().expect("valid request");
        let cookies = Self::get_cookies(vault.vault()).await;
        let cookies = HeaderValue::from_str(&cookies).expect("valid cookies");
        request.headers_mut().append(header::COOKIE, cookies);

        match tokio_tungstenite::connect_async(request).await {
            Ok((ws, response)) => {
                Self::update_cookies(vault.vault(), &response).await;
                Ok(Some(conn::wrap(ws)))
            }
            Err(tungstenite::Error::Http(resp)) if resp.status().is_client_error() => {
                bail!("room {name} doesn't exist");
            }
            Err(tungstenite::Error::Url(_) | tungstenite::Error::HttpFormat(_)) => {
                bail!("format error for room {name}");
            }
            Err(_) => Ok(None),
        }
    }

    async fn regularly_request_logs(event_tx: &mpsc::UnboundedSender<Event>) {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await; // TODO Make configurable
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
                Event::Nick(name) => self.on_nick(name),
                Event::Send(parent, content) => self.on_send(parent, content),
            }
        }
        Ok(())
    }

    async fn on_data(&mut self, data: Data) -> anyhow::Result<()> {
        match data {
            Data::BounceEvent(_) => {}
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
                } else {
                    bail!("send event before snapshot event");
                }
            }
            Data::SnapshotEvent(d) => {
                info!("e&{}: successfully joined", self.name);
                self.vault.join(OffsetDateTime::now_utc());
                self.last_msg_id = Some(d.log.last().map(|m| m.id));
                self.vault.add_messages(d.log, None);
            }
            Data::LogReply(d) => {
                self.vault.add_messages(d.log, d.before);
            }
            Data::SendReply(d) => {
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    let id = d.0.id;
                    self.vault.add_message(d.0, *last_msg_id);
                    *last_msg_id = Some(id);
                } else {
                    bail!("send reply before snapshot event");
                }
            }
            _ => {}
        }
        let _ = self.ui_event_tx.send(UiEvent::Redraw);
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

    fn on_nick(&self, name: String) {
        if let Some(conn_tx) = &self.conn_tx {
            let conn_tx = conn_tx.clone();
            task::spawn(async move {
                let _ = conn_tx.send(Nick { name }).await;
            });
        }
    }

    fn on_send(&self, parent: Option<Snowflake>, content: String) {
        if let Some(conn_tx) = &self.conn_tx {
            let conn_tx = conn_tx.clone();
            task::spawn(async move {
                let _ = conn_tx.send(Send { content, parent }).await;
            });
        }
    }
}

#[derive(Debug)]
pub struct Room {
    #[allow(dead_code)]
    canary: oneshot::Sender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl Room {
    pub fn new(vault: EuphVault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        let (canary_tx, canary_rx) = oneshot::channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let state = State {
            name: vault.room().to_string(),
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

    pub fn stopped(&self) -> bool {
        self.event_tx.is_closed()
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

    pub fn nick(&self, name: String) -> Result<(), Error> {
        self.event_tx
            .send(Event::Nick(name))
            .map_err(|_| Error::Stopped)
    }

    pub fn send(&self, parent: Option<Snowflake>, content: String) -> Result<(), Error> {
        self.event_tx
            .send(Event::Send(parent, content))
            .map_err(|_| Error::Stopped)
    }
}
