//! Connection state modeling.

use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Duration;

use anyhow::bail;
use chrono::Utc;
use futures::channel::oneshot;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use rand::Rng;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::{select, task, time};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

use crate::replies::{self, PendingReply, Replies};

use super::api::packet::{Command, Packet, ParsedPacket};
use super::api::{
    BounceEvent, Data, HelloEvent, PersonalAccountView, Ping, PingReply, SnapshotEvent,
};
use super::{SessionView, Time, UserId};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection closed")]
    ConnectionClosed,
    #[error("packet timed out")]
    TimedOut,
    #[error("incorrect reply type")]
    IncorrectReplyType,
    #[error("{0}")]
    Euph(String),
}

#[derive(Debug)]
enum Event {
    Message(tungstenite::Message),
    SendCmd(Data, oneshot::Sender<PendingReply<Result<Data, String>>>),
    SendRpl(Option<String>, Data),
    Status(oneshot::Sender<Status>),
    DoPings,
}

impl Event {
    fn send_cmd<C: Into<Data>>(
        cmd: C,
        rpl: oneshot::Sender<PendingReply<Result<Data, String>>>,
    ) -> Self {
        Self::SendCmd(cmd.into(), rpl)
    }

    fn send_rpl<C: Into<Data>>(id: Option<String>, rpl: C) -> Self {
        Self::SendRpl(id, rpl.into())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Joining {
    hello: Option<HelloEvent>,
    snapshot: Option<SnapshotEvent>,
    bounce: Option<BounceEvent>,
}

impl Joining {
    fn on_data(&mut self, data: Data) -> anyhow::Result<()> {
        match data {
            Data::BounceEvent(p) => self.bounce = Some(p),
            Data::HelloEvent(p) => self.hello = Some(p),
            Data::SnapshotEvent(p) => self.snapshot = Some(p),
            _ => {}
        }
        Ok(())
    }

    fn joined(&self) -> Option<Joined> {
        if let (Some(hello), Some(snapshot)) = (&self.hello, &self.snapshot) {
            let listing = snapshot
                .listing
                .iter()
                .cloned()
                .map(|s| (s.id.clone(), s))
                .collect::<HashMap<_, _>>();
            Some(Joined {
                session: hello.session.clone(),
                account: hello.account.clone(),
                listing,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct Joined {
    session: SessionView,
    account: Option<PersonalAccountView>,
    listing: HashMap<UserId, SessionView>,
}

impl Joined {
    fn on_data(&mut self, data: Data) -> anyhow::Result<()> {
        match data {
            Data::JoinEvent(p) => {
                self.listing.insert(p.0.id.clone(), p.0);
            }
            Data::SendEvent(p) => {
                self.listing.insert(p.0.sender.id.clone(), p.0.sender);
            }
            Data::PartEvent(p) => {
                self.listing.remove(&p.0.id);
            }
            Data::NetworkEvent(p) => {
                if p.r#type == "partition" {
                    self.listing.retain(|_, s| {
                        !(s.server_id == p.server_id && s.server_era == p.server_era)
                    });
                }
            }
            Data::NickEvent(p) => {
                if let Some(session) = self.listing.get_mut(&p.id) {
                    session.name = p.to;
                }
            }
            Data::NickReply(p) => {
                assert_eq!(self.session.id, p.id);
                self.session.name = p.to;
            }
            // The who reply is broken and can't be trusted right now, so we'll
            // not even look at it.
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Status {
    Joining(Joining),
    Joined(Joined),
}

struct State {
    ws_tx: SplitSink<WsStream, tungstenite::Message>,
    last_id: usize,
    replies: Replies<String, Result<Data, String>>,

    packet_tx: mpsc::UnboundedSender<Data>,

    last_ws_ping: Option<Vec<u8>>,
    last_ws_pong: Option<Vec<u8>>,
    last_euph_ping: Option<Time>,
    last_euph_pong: Option<Time>,

    status: Status,
}

impl State {
    async fn run(
        ws: WsStream,
        tx_canary: oneshot::Receiver<Infallible>,
        rx_canary: oneshot::Receiver<Infallible>,
        event_tx: mpsc::UnboundedSender<Event>,
        mut event_rx: mpsc::UnboundedReceiver<Event>,
        packet_tx: mpsc::UnboundedSender<Data>,
    ) {
        let (ws_tx, mut ws_rx) = ws.split();
        let state = Self {
            ws_tx,
            last_id: 0,
            replies: Replies::new(Duration::from_secs(10)), // TODO Make configurable
            packet_tx,
            last_ws_ping: None,
            last_ws_pong: None,
            last_euph_ping: None,
            last_euph_pong: None,
            status: Status::Joining(Joining::default()),
        };

        select! {
            _ = tx_canary => (),
            _ = rx_canary => (),
            _ = Self::listen(&mut ws_rx, &event_tx) => (),
            _ = Self::send_ping_events(&event_tx) => (),
            _ = state.handle_events(&event_tx, &mut event_rx) => (),
        }
    }

    async fn listen(
        ws_rx: &mut SplitStream<WsStream>,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = ws_rx.next().await {
            event_tx.send(Event::Message(msg?))?;
        }
        Ok(())
    }

    async fn send_ping_events(event_tx: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
        loop {
            event_tx.send(Event::DoPings)?;
            time::sleep(Duration::from_secs(10)).await; // TODO Make configurable
        }
    }

    async fn handle_events(
        mut self,
        event_tx: &mpsc::UnboundedSender<Event>,
        event_rx: &mut mpsc::UnboundedReceiver<Event>,
    ) -> anyhow::Result<()> {
        while let Some(ev) = event_rx.recv().await {
            match ev {
                Event::Message(msg) => self.on_msg(msg, event_tx)?,
                Event::SendCmd(data, reply_tx) => self.on_send_cmd(data, reply_tx).await?,
                Event::SendRpl(id, data) => self.on_send_rpl(id, data).await?,
                Event::Status(reply_tx) => self.on_status(reply_tx),
                Event::DoPings => self.do_pings(event_tx).await?,
            }
        }
        Ok(())
    }

    fn on_msg(
        &mut self,
        msg: tungstenite::Message,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<()> {
        match msg {
            tungstenite::Message::Text(t) => self.on_packet(serde_json::from_str(&t)?, event_tx)?,
            tungstenite::Message::Binary(_) => bail!("unexpected binary message"),
            tungstenite::Message::Ping(_) => {}
            tungstenite::Message::Pong(p) => self.last_ws_pong = Some(p),
            tungstenite::Message::Close(_) => {}
            tungstenite::Message::Frame(_) => {}
        }
        Ok(())
    }

    fn on_packet(
        &mut self,
        packet: Packet,
        event_tx: &mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<()> {
        let packet = ParsedPacket::from_packet(packet)?;

        // Complete pending replies if the packet has an id
        if let Some(id) = &packet.id {
            self.replies.complete(id, packet.content.clone());
        }

        // Shovel events into self.packet_tx, assuming that no event ever
        // errors. Events with errors are simply ignored.
        if let Ok(data) = &packet.content {
            if data.is_event() {
                self.packet_tx.send(data.clone())?;
            }
        }

        // Play a game of table tennis
        match &packet.content {
            Ok(Data::PingReply(p)) => self.last_euph_pong = p.time,
            Ok(Data::PingEvent(p)) => {
                let reply = PingReply { time: Some(p.time) };
                event_tx.send(Event::send_rpl(packet.id.clone(), reply))?;
            }
            // TODO Handle disconnect event?
            _ => {}
        }

        // Update internal state
        if let Ok(data) = packet.content {
            match &mut self.status {
                Status::Joining(joining) => {
                    joining.on_data(data)?;
                    if let Some(joined) = joining.joined() {
                        self.status = Status::Joined(joined);
                    }
                }
                Status::Joined(joined) => joined.on_data(data)?,
            }
        }

        Ok(())
    }

    async fn on_send_cmd(
        &mut self,
        data: Data,
        reply_tx: oneshot::Sender<PendingReply<Result<Data, String>>>,
    ) -> anyhow::Result<()> {
        // Overkill of universe-heat-death-like proportions
        self.last_id = self.last_id.wrapping_add(1);
        let id = format!("{}", self.last_id);

        let packet = ParsedPacket {
            id: Some(id.clone()),
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
        .to_packet()?;

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws_tx.send(msg).await?;

        let _ = reply_tx.send(self.replies.wait_for(id));

        Ok(())
    }

    async fn on_send_rpl(&mut self, id: Option<String>, data: Data) -> anyhow::Result<()> {
        let packet = ParsedPacket {
            id,
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
        .to_packet()?;

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws_tx.send(msg).await?;

        Ok(())
    }

    fn on_status(&mut self, reply_tx: oneshot::Sender<Status>) {
        let _ = reply_tx.send(self.status.clone());
    }

    async fn do_pings(&mut self, event_tx: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
        // Check old ws ping
        if self.last_ws_ping.is_some() && self.last_ws_ping != self.last_ws_pong {
            bail!("server missed ws ping")
        }

        // Send new ws ping
        let mut ws_payload = [0_u8; 8];
        rand::thread_rng().fill(&mut ws_payload);
        self.ws_tx
            .send(tungstenite::Message::Ping(ws_payload.to_vec()))
            .await?;

        // Check old euph ping
        if self.last_euph_ping.is_some() && self.last_euph_ping != self.last_euph_pong {
            bail!("server missed euph ping")
        }

        // Send new euph ping
        let euph_payload = Time(Utc::now());
        let (tx, _) = oneshot::channel();
        event_tx.send(Event::send_cmd(Ping { time: euph_payload }, tx))?;

        Ok(())
    }
}

pub struct ConnTx {
    canary: oneshot::Sender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl ConnTx {
    pub async fn send<C>(&self, cmd: C) -> Result<C::Reply, Error>
    where
        C: Command + Into<Data>,
        C::Reply: TryFrom<Data, Error = ()>,
    {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::SendCmd(cmd.into(), tx))
            .map_err(|_| Error::ConnectionClosed)?;
        let pending_reply = rx
            .await
            // This should only happen if something goes wrong during encoding
            // of the packet or while sending it through the websocket. Assuming
            // the first doesn't happen, the connection is probably closed.
            .map_err(|_| Error::ConnectionClosed)?;
        let data = pending_reply
            .get()
            .await
            .map_err(|e| match e {
                replies::Error::TimedOut => Error::TimedOut,
                replies::Error::Canceled => Error::ConnectionClosed,
            })?
            .map_err(Error::Euph)?;
        data.try_into().map_err(|_| Error::IncorrectReplyType)
    }

    pub async fn status(&self) -> Result<Status, Error> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Status(tx))
            .map_err(|_| Error::ConnectionClosed)?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }
}

pub struct ConnRx {
    canary: oneshot::Sender<Infallible>,
    packet_rx: mpsc::UnboundedReceiver<Data>,
}

impl ConnRx {
    pub async fn recv(&mut self) -> Result<Data, Error> {
        self.packet_rx.recv().await.ok_or(Error::ConnectionClosed)
    }
}

// TODO Combine ConnTx and ConnRx and implement Stream + Sink?

pub fn wrap(ws: WsStream) -> (ConnTx, ConnRx) {
    let (tx_canary_tx, tx_canary_rx) = oneshot::channel();
    let (rx_canary_tx, rx_canary_rx) = oneshot::channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (packet_tx, packet_rx) = mpsc::unbounded_channel();

    task::spawn(State::run(
        ws,
        tx_canary_rx,
        rx_canary_rx,
        event_tx.clone(),
        event_rx,
        packet_tx,
    ));

    let tx = ConnTx {
        canary: tx_canary_tx,
        event_tx,
    };
    let rx = ConnRx {
        canary: rx_canary_tx,
        packet_rx,
    };
    (tx, rx)
}
