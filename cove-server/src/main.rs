use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::anyhow;
use cove_core::packets::{Cmd, HelloRpl, Packet, Rpl};
use cove_core::{Identity, MessageId, Session, SessionId};
use futures::stream::{SplitSink, SplitStream};
use futures::{future, Sink, SinkExt, Stream, StreamExt, TryStreamExt};
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

#[derive(Debug, Clone)]
struct Server {
    rooms: Arc<RwLock<HashMap<String, Arc<Mutex<Room>>>>>,
}

impl Server {
    fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
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
    ) -> anyhow::Result<(String, String, Identity, u64)> {
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
        let identity = Identity::of(&cmd.identity);
        Ok((cmd.room, cmd.nick, identity, id))
    }

    async fn on_conn(self, stream: TcpStream) {
        println!("Connection from {}", stream.peer_addr().unwrap());
        let stream = tokio_tungstenite::accept_async(stream).await.unwrap();
        let (mut tx, mut rx) = stream.split();
        let (room, nick, identity, id) = match self.greet(&mut tx, &mut rx).await {
            Ok(info) => info,
            Err(_) => return,
        };
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
