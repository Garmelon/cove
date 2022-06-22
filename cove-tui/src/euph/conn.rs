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

use crate::replies::{self, Replies};

use super::api::{
    BounceEvent, FromPacket, HelloEvent, JoinEvent, NetworkEvent, NickEvent, NickReply, Packet,
    PacketType, PartEvent, PersonalAccountView, Ping, PingEvent, PingReply, SendEvent,
    SnapshotEvent, ToPacket,
};
use super::{SessionView, Time, UserId};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection closed")]
    ConnectionClosed,
    #[error("packet timed out")]
    TimedOut,
}

#[derive(Debug)]
enum Event {
    Message(tungstenite::Message),
    Send(Packet, oneshot::Sender<Result<Packet, Error>>),
    Status(oneshot::Sender<Status>),
    DoPings,
}

#[derive(Debug, Clone, Default)]
pub struct Joining {
    hello: Option<HelloEvent>,
    snapshot: Option<SnapshotEvent>,
    bounce: Option<BounceEvent>,
}

impl Joining {
    fn on_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        match packet.r#type {
            PacketType::BounceEvent => self.bounce = Some(BounceEvent::from_packet(packet)?),
            PacketType::HelloEvent => self.hello = Some(HelloEvent::from_packet(packet)?),
            PacketType::SnapshotEvent => self.snapshot = Some(SnapshotEvent::from_packet(packet)?),
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
    fn on_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        match packet.r#type {
            PacketType::JoinEvent => {
                let packet = JoinEvent::from_packet(packet)?;
                self.listing.insert(packet.0.id.clone(), packet.0);
            }
            PacketType::SendEvent => {
                let packet = SendEvent::from_packet(packet)?;
                self.listing
                    .insert(packet.0.sender.id.clone(), packet.0.sender);
            }
            PacketType::PartEvent => {
                let packet = PartEvent::from_packet(packet)?;
                self.listing.remove(&packet.0.id);
            }
            PacketType::NetworkEvent => {
                let p = NetworkEvent::from_packet(packet)?;
                if p.r#type == "partition" {
                    self.listing.retain(|_, s| {
                        !(s.server_id == p.server_id && s.server_era == p.server_era)
                    });
                }
            }
            PacketType::NickEvent => {
                let packet = NickEvent::from_packet(packet)?;
                if let Some(session) = self.listing.get_mut(&packet.id) {
                    session.name = packet.to;
                }
            }
            PacketType::NickReply => {
                // Since this is a reply, it may contain errors, for example if
                // the user specified an invalid nick. We can't just die if that
                // happens, so we ignore the error case.
                if let Ok(packet) = NickReply::from_packet(packet) {
                    assert_eq!(self.session.id, packet.id);
                    self.session.name = packet.to;
                }
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
    replies: Replies<String, Packet>,

    packet_tx: mpsc::UnboundedSender<Packet>,

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
        packet_tx: mpsc::UnboundedSender<Packet>,
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
                Event::Send(packet, reply_tx) => self.on_send(packet, reply_tx).await?,
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
        if packet.r#type == PacketType::PingReply {
            let packet = PingReply::from_packet(packet.clone())?;
            self.last_euph_pong = packet.time;
        } else if packet.r#type == PacketType::PingEvent {
            let time = Some(PingEvent::from_packet(packet.clone())?.time);
            Self::send_unconditionally(event_tx, PingReply { time }, packet.id.clone())?;
        }

        if let Some(id) = &packet.id {
            self.replies.complete(id, packet.clone());
        }

        self.packet_tx.send(packet.clone())?;

        // TODO Handle disconnect event?

        match &mut self.status {
            Status::Joining(joining) => {
                joining.on_packet(packet)?;
                if let Some(joined) = joining.joined() {
                    self.status = Status::Joined(joined);
                }
            }
            Status::Joined(joined) => joined.on_packet(packet)?,
        }

        Ok(())
    }

    async fn on_send(
        &mut self,
        mut packet: Packet,
        reply_tx: oneshot::Sender<Result<Packet, Error>>,
    ) -> anyhow::Result<()> {
        let id = if let Some(id) = packet.id.clone() {
            id
        } else {
            // Overkill of universe-heat-death-like proportions
            self.last_id = self.last_id.wrapping_add(1);
            format!("{}", self.last_id)
        };
        packet.id = Some(id.clone());

        let pending_reply = self.replies.wait_for(id);

        let msg = tungstenite::Message::Text(serde_json::to_string(&packet)?);
        self.ws_tx.send(msg).await?;

        let reply = match pending_reply.get().await {
            Ok(reply) => Ok(reply),
            Err(replies::Error::TimedOut) => Err(Error::TimedOut),
            // We could also send an Error::ConnectionClosed here, but that
            // happens automatically in the send function once we drop reply_tx.
            Err(replies::Error::Canceled) => return Ok(()),
        };
        let _ = reply_tx.send(reply);

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
        Self::send_unconditionally(event_tx, Ping { time: euph_payload }, None)?;

        Ok(())
    }

    fn send_unconditionally<T: ToPacket>(
        event_tx: &mpsc::UnboundedSender<Event>,
        packet: T,
        id: Option<String>,
    ) -> anyhow::Result<()> {
        let (tx, _) = oneshot::channel();
        event_tx.send(Event::Send(packet.to_packet(id), tx))?;
        Ok(())
    }
}

pub struct ConnTx {
    canary: oneshot::Sender<Infallible>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl ConnTx {
    pub async fn send<T: ToPacket>(&self, packet: T) -> Result<Packet, Error> {
        let (tx, rx) = oneshot::channel();
        let event = Event::Send(packet.to_packet(None), tx);
        self.event_tx
            .send(event)
            .map_err(|_| Error::ConnectionClosed)?;
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(Error::ConnectionClosed),
        }
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
    packet_rx: mpsc::UnboundedReceiver<Packet>,
}

impl ConnRx {
    pub async fn recv(&mut self) -> Result<Packet, Error> {
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
