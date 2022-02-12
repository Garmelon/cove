mod conn;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use cove_core::packets::{Cmd, HelloRpl, Packet, Rpl};
use cove_core::{Identity, MessageId, Session, SessionId};
use futures::stream::{SplitSink, SplitStream};
use futures::{future, Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use rand::prelude::ThreadRng;
use rand::Rng;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::{self, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message as TkMessage;
use tokio_tungstenite::WebSocketStream;

#[derive(Debug)]
struct Client {
    session: Session,
    packets: UnboundedSender<Packet>,
}

#[derive(Debug)]
struct Room {
    clients: HashMap<SessionId, Client>,
    last_message: MessageId,
    last_timestamp: u128,
}

impl Room {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
            last_message: MessageId::of(&format!("{}", rand::thread_rng().gen::<u64>())),
            last_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("executed after 1970")
                .as_millis(),
        }
    }
}

#[derive(Debug, Clone)]
struct Server {
    rooms: Arc<Mutex<HashMap<String, Arc<Mutex<Room>>>>>,
}

impl Server {
    fn new() -> Self {
        Self {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn room(&self, name: String) -> Arc<Mutex<Room>> {
        self.rooms
            .lock()
            .await
            .entry(name)
            .or_insert_with(|| Arc::new(Mutex::new(Room::new())))
            .clone()
    }

    async fn recv(rx: &mut SplitStream<WebSocketStream<TcpStream>>) -> anyhow::Result<Packet> {
        loop {
            let msg = rx.next().await.ok_or(anyhow!("connection closed"))??;
            let str = match msg {
                TkMessage::Text(str) => str,
                TkMessage::Ping(_) | TkMessage::Pong(_) => continue,
                TkMessage::Binary(_) => return Err(anyhow!("invalid binary packet")),
                TkMessage::Close(_) => return Err(anyhow!("connection closed")),
            };
            break Ok(serde_json::from_str(&str)?);
        }
    }

    async fn send(
        tx: &mut SplitSink<WebSocketStream<TcpStream>, TkMessage>,
        packet: &Packet,
    ) -> anyhow::Result<()> {
        let str = serde_json::to_string(packet).expect("serialisable packet");
        let msg = TkMessage::Text(str);
        tx.feed(msg).await?;
        tx.flush().await?;
        Ok(())
    }

    fn check_room(room: &str) -> Option<String> {
        if !room.is_empty() {
            return Some("is empty".to_string());
        }
        if !room.is_ascii() {
            return Some("contains non-ascii characters".to_string());
        }
        if room.len() > 1024 {
            return Some("contains more than 1024 characters".to_string());
        }
        if !room
            .chars()
            .all(|c| c == '-' || c == '.' || ('a'..='z').contains(&c))
        {
            return Some("must only contain a-z, '-' and '_'".to_string());
        }
        None
    }

    fn check_nick(nick: &str) -> Option<String> {
        if !nick.is_empty() {
            return Some("is empty".to_string());
        }
        if !nick.trim().is_empty() {
            return Some("contains only whitespace".to_string());
        }
        let nick = nick.trim();
        if nick.chars().count() > 1024 {
            return Some("contains more than 1024 characters".to_string());
        }
        None
    }

    fn check_identity(identity: &str) -> Option<String> {
        if identity.chars().count() > 32768 {
            return Some("contains more than 32768 characters".to_string());
        }
        None
    }

    async fn greet(
        &self,
        tx: &mut SplitSink<WebSocketStream<TcpStream>, TkMessage>,
        rx: &mut SplitStream<WebSocketStream<TcpStream>>,
    ) -> anyhow::Result<(String, Session, u64)> {
        // TODO Allow multiple Hello commands until the first succeeds
        let packet = Self::recv(rx).await?;
        let (id, cmd) = match packet {
            Packet::Cmd {
                id,
                cmd: Cmd::Hello(cmd),
            } => (id, cmd),
            _ => return Err(anyhow!("not a hello command")),
        };
        if let Some(reason) = Self::check_room(&cmd.room) {
            Self::send(tx, &Packet::rpl(id, HelloRpl::InvalidRoom { reason })).await?;
            return Err(anyhow!("invalid room"));
        }
        if let Some(reason) = Self::check_nick(&cmd.nick) {
            Self::send(tx, &Packet::rpl(id, HelloRpl::InvalidNick { reason })).await?;
            return Err(anyhow!("invalid nick"));
        }
        if let Some(reason) = Self::check_identity(&cmd.identity) {
            Self::send(tx, &Packet::rpl(id, HelloRpl::InvalidNick { reason })).await?;
            return Err(anyhow!("invalid identity"));
        }
        let session = Session {
            id: SessionId::of(&format!("{}", rand::thread_rng().gen::<u64>())),
            nick: cmd.nick,
            identity: Identity::of(&cmd.identity),
        };
        Ok((cmd.room, session, id))
    }

    async fn on_conn(self, stream: TcpStream) {
        // TODO Ping-pong starting from the beginning (not just after hello)
        println!("Connection from {}", stream.peer_addr().unwrap());
        let stream = tokio_tungstenite::accept_async(stream).await.unwrap();
        let (mut tx, mut rx) = stream.split();
        let (room, session, id) = match self.greet(&mut tx, &mut rx).await {
            Ok(info) => info,
            Err(_) => return,
        };
        let room = self.room(room).await;
        let (packets, client_rx) = mpsc::unbounded_channel();
        {
            let mut room = room.lock().await;
            packets.send(Packet::rpl(
                id,
                HelloRpl::Success {
                    you: session.clone(),
                    others: room.clients.values().map(|c| c.session.clone()).collect(),
                    last_message: room.last_message,
                },
            ));
            let client = Client { session, packets };
            room.clients.insert(client.session.id, client);
        }
        todo!()
    }
}

#[tokio::main]
async fn main() {
    let server = Server::new();
    let listener = TcpListener::bind(("::0", 40080)).await.unwrap();
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(server.clone().on_conn(stream));
    }
}
