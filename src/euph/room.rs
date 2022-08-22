use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use cookie::{Cookie, CookieJar};
use euphoxide::api::packet::ParsedPacket;
use euphoxide::api::{
    Auth, AuthOption, Data, Log, Login, Logout, Nick, Send, Snowflake, Time, UserId,
};
use euphoxide::conn::{ConnRx, ConnTx, Joining, Status};
use log::{error, info, warn};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task};
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue};

use crate::macros::ok_or_return;
use crate::vault::{EuphVault, Vault};

const TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const LOG_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("room stopped")]
    Stopped,
}

pub enum EuphRoomEvent {
    Connected,
    Disconnected,
    Packet(Box<ParsedPacket>),
    Stopped,
}

#[derive(Debug)]
enum Event {
    // Events
    Connected(ConnTx),
    Disconnected,
    Packet(Box<ParsedPacket>),
    // Commands
    Status(oneshot::Sender<Option<Status>>),
    RequestLogs,
    Auth(String),
    Nick(String),
    Send(Option<Snowflake>, String, oneshot::Sender<Snowflake>),
    Login { email: String, password: String },
    Logout,
}

#[derive(Debug)]
struct State {
    name: String,
    vault: EuphVault,

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
        euph_room_event_tx: mpsc::UnboundedSender<EuphRoomEvent>,
    ) {
        let vault = self.vault.clone();
        let name = self.name.clone();
        let result = select! {
            _ = canary => Ok(()),
            _ = Self::reconnect(&vault, &name, &event_tx) => Ok(()),
            _ = Self::regularly_request_logs(&event_tx) => Ok(()),
            e = self.handle_events(&mut event_rx, &euph_room_event_tx) => e,
        };

        if let Err(e) = result {
            error!("e&{name}: {}", e);
        }

        // Ensure that whoever is using this room knows that it's gone.
        // Otherwise, the users of the Room may be left in an inconsistent or
        // outdated state, and the UI may not update correctly.
        let _ = euph_room_event_tx.send(EuphRoomEvent::Stopped);
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

                while let Some(packet) = conn_rx.recv().await {
                    event_tx.send(Event::Packet(Box::new(packet)))?;
                }

                info!("e&{}: disconnected", name);
                event_tx.send(Event::Disconnected)?;
            } else {
                info!("e&{}: could not connect", name);
                event_tx.send(Event::Disconnected)?;
            }
            tokio::time::sleep(RECONNECT_INTERVAL).await;
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

    fn update_cookies(vault: &Vault, response: &Response) {
        let mut cookie_jar = CookieJar::new();

        for (name, value) in response.headers() {
            if name == header::SET_COOKIE {
                let value_str = ok_or_return!(value.to_str());
                let cookie = ok_or_return!(Cookie::from_str(value_str));
                cookie_jar.add(cookie);
            }
        }

        vault.set_euph_cookies(cookie_jar);
    }

    async fn connect(vault: &EuphVault, name: &str) -> anyhow::Result<Option<(ConnTx, ConnRx)>> {
        let uri = format!("wss://euphoria.io/room/{name}/ws?h=1");
        let mut request = uri.into_client_request().expect("valid request");
        let cookies = Self::get_cookies(vault.vault()).await;
        let cookies = HeaderValue::from_str(&cookies).expect("valid cookies");
        request.headers_mut().append(header::COOKIE, cookies);

        match tokio_tungstenite::connect_async(request).await {
            Ok((ws, response)) => {
                Self::update_cookies(vault.vault(), &response);
                Ok(Some(euphoxide::wrap(ws, TIMEOUT)))
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
            tokio::time::sleep(LOG_INTERVAL).await;
            let _ = event_tx.send(Event::RequestLogs);
        }
    }

    async fn handle_events(
        &mut self,
        event_rx: &mut mpsc::UnboundedReceiver<Event>,
        euph_room_event_tx: &mpsc::UnboundedSender<EuphRoomEvent>,
    ) -> anyhow::Result<()> {
        while let Some(event) = event_rx.recv().await {
            match event {
                Event::Connected(conn_tx) => {
                    self.conn_tx = Some(conn_tx);
                    let _ = euph_room_event_tx.send(EuphRoomEvent::Connected);
                }
                Event::Disconnected => {
                    self.conn_tx = None;
                    self.last_msg_id = None;
                    let _ = euph_room_event_tx.send(EuphRoomEvent::Disconnected);
                }
                Event::Packet(packet) => {
                    self.on_packet(&*packet).await?;
                    let _ = euph_room_event_tx.send(EuphRoomEvent::Packet(packet));
                }
                Event::Status(reply_tx) => self.on_status(reply_tx).await,
                Event::RequestLogs => self.on_request_logs(),
                Event::Auth(password) => self.on_auth(password),
                Event::Nick(name) => self.on_nick(name),
                Event::Send(parent, content, id_tx) => self.on_send(parent, content, id_tx),
                Event::Login { email, password } => self.on_login(email, password),
                Event::Logout => self.on_logout(),
            }
        }
        Ok(())
    }

    async fn own_user_id(&self) -> Option<UserId> {
        Some(match self.conn_tx.as_ref()?.status().await.ok()? {
            Status::Joining(Joining { hello, .. }) => hello?.session.id,
            Status::Joined(joined) => joined.session.id,
        })
    }

    async fn on_packet(&mut self, packet: &ParsedPacket) -> anyhow::Result<()> {
        let data = ok_or_return!(&packet.content, Ok(()));
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
                let own_user_id = self.own_user_id().await;
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    let id = d.0.id;
                    self.vault
                        .add_message(d.0.clone(), *last_msg_id, own_user_id);
                    *last_msg_id = Some(id);
                } else {
                    bail!("send event before snapshot event");
                }
            }
            Data::SnapshotEvent(d) => {
                info!("e&{}: successfully joined", self.name);
                self.vault.join(Time::now());
                self.last_msg_id = Some(d.log.last().map(|m| m.id));
                let own_user_id = self.own_user_id().await;
                self.vault.add_messages(d.log.clone(), None, own_user_id);
            }
            Data::LogReply(d) => {
                let own_user_id = self.own_user_id().await;
                self.vault
                    .add_messages(d.log.clone(), d.before, own_user_id);
            }
            Data::SendReply(d) => {
                let own_user_id = self.own_user_id().await;
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    let id = d.0.id;
                    self.vault
                        .add_message(d.0.clone(), *last_msg_id, own_user_id);
                    *last_msg_id = Some(id);
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

    fn on_auth(&self, password: String) {
        if let Some(conn_tx) = &self.conn_tx {
            let conn_tx = conn_tx.clone();
            task::spawn(async move {
                let _ = conn_tx
                    .send(Auth {
                        r#type: AuthOption::Passcode,
                        passcode: Some(password),
                    })
                    .await;
            });
        }
    }

    fn on_nick(&self, name: String) {
        if let Some(conn_tx) = &self.conn_tx {
            let conn_tx = conn_tx.clone();
            task::spawn(async move {
                let _ = conn_tx.send(Nick { name }).await;
            });
        }
    }

    fn on_send(
        &self,
        parent: Option<Snowflake>,
        content: String,
        id_tx: oneshot::Sender<Snowflake>,
    ) {
        if let Some(conn_tx) = &self.conn_tx {
            let conn_tx = conn_tx.clone();
            task::spawn(async move {
                if let Ok(reply) = conn_tx.send(Send { content, parent }).await {
                    let _ = id_tx.send(reply.0.id);
                }
            });
        }
    }

    fn on_login(&self, email: String, password: String) {
        if let Some(conn_tx) = &self.conn_tx {
            let _ = conn_tx.send(Login {
                namespace: "email".to_string(),
                id: email,
                password,
            });
        }
    }

    fn on_logout(&self) {
        if let Some(conn_tx) = &self.conn_tx {
            let _ = conn_tx.send(Logout);
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
    pub fn new(vault: EuphVault) -> (Self, mpsc::UnboundedReceiver<EuphRoomEvent>) {
        let (canary_tx, canary_rx) = oneshot::channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (euph_room_event_tx, euph_room_event_rx) = mpsc::unbounded_channel();

        let state = State {
            name: vault.room().to_string(),
            vault,
            conn_tx: None,
            last_msg_id: None,
            requesting_logs: Arc::new(Mutex::new(false)),
        };

        task::spawn(state.run(canary_rx, event_tx.clone(), event_rx, euph_room_event_tx));

        let new_room = Self {
            canary: canary_tx,
            event_tx,
        };
        (new_room, euph_room_event_rx)
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

    pub fn auth(&self, password: String) -> Result<(), Error> {
        self.event_tx
            .send(Event::Auth(password))
            .map_err(|_| Error::Stopped)
    }

    pub fn nick(&self, name: String) -> Result<(), Error> {
        self.event_tx
            .send(Event::Nick(name))
            .map_err(|_| Error::Stopped)
    }

    pub fn send(
        &self,
        parent: Option<Snowflake>,
        content: String,
    ) -> Result<oneshot::Receiver<Snowflake>, Error> {
        let (id_tx, id_rx) = oneshot::channel();
        self.event_tx
            .send(Event::Send(parent, content, id_tx))
            .map(|_| id_rx)
            .map_err(|_| Error::Stopped)
    }

    pub fn login(&self, email: String, password: String) -> Result<(), Error> {
        self.event_tx
            .send(Event::Login { email, password })
            .map_err(|_| Error::Stopped)
    }

    pub fn logout(&self) -> Result<(), Error> {
        self.event_tx
            .send(Event::Logout)
            .map_err(|_| Error::Stopped)
    }
}
