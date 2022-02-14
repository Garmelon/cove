// TODO Logging

mod conn;
mod util;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use conn::{ConnMaintenance, ConnRx, ConnTx};
use cove_core::packets::{
    Cmd, HelloCmd, HelloRpl, JoinNtf, NickCmd, NickNtf, NickRpl, Packet, PartNtf, SendCmd, SendNtf,
    SendRpl, WhoCmd, WhoRpl,
};
use cove_core::{Identity, Message, MessageId, Session, SessionId};
use log::{info, warn};
use rand::Rng;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct Client {
    session: Session,
    send: ConnTx,
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
            last_timestamp: util::timestamp(),
        }
    }

    fn client(&self, id: SessionId) -> &Client {
        self.clients.get(&id).expect("invalid session id")
    }

    fn client_mut(&mut self, id: SessionId) -> &mut Client {
        self.clients.get_mut(&id).expect("invalid session id")
    }

    fn notify_all(&self, packet: &Packet) {
        for client in self.clients.values() {
            let _ = client.send.send(packet);
        }
    }

    fn notify_except(&self, id: SessionId, packet: &Packet) {
        for client in self.clients.values() {
            if client.session.id != id {
                let _ = client.send.send(packet);
            }
        }
    }

    fn join(&mut self, client: Client) {
        if self.clients.contains_key(&client.session.id) {
            // Session ids are generated randomly and a collision should be very
            // unlikely.
            panic!("duplicated session id");
        }

        self.notify_all(&Packet::ntf(JoinNtf {
            who: client.session.clone(),
        }));

        self.clients.insert(client.session.id, client);
    }

    fn part(&mut self, id: SessionId) {
        let client = self.clients.remove(&id).expect("invalid session id");

        self.notify_all(&Packet::ntf(PartNtf {
            who: client.session,
        }));
    }

    fn nick(&mut self, id: SessionId, nick: String) {
        let who = {
            let client = self.client_mut(id);
            client.session.nick = nick;
            client.session.clone()
        };

        self.notify_except(id, &Packet::ntf(NickNtf { who }))
    }

    fn send(&mut self, id: SessionId, parent: Option<MessageId>, content: String) -> Message {
        let client = &self.clients[&id];

        self.last_timestamp = util::timestamp_after(self.last_timestamp);

        let message = Message {
            time: self.last_timestamp,
            pred: self.last_message,
            parent,
            identity: client.session.identity,
            nick: client.session.nick.clone(),
            content,
        };

        self.notify_except(
            id,
            &Packet::ntf(SendNtf {
                message: message.clone(),
            }),
        );

        message
    }

    fn who(&self, id: SessionId) -> (Session, Vec<Session>) {
        let session = self.client(id).session.clone();
        let others = self
            .clients
            .values()
            .filter(|client| client.session.id != id)
            .map(|client| client.session.clone())
            .collect();
        (session, others)
    }
}

#[derive(Debug)]
struct ServerSession {
    tx: ConnTx,
    rx: ConnRx,
    room: Arc<Mutex<Room>>,
    session: Session,
}

impl ServerSession {
    async fn handle_nick(&mut self, id: u64, cmd: NickCmd) -> anyhow::Result<()> {
        if let Some(reason) = util::check_nick(&cmd.nick) {
            self.tx
                .send(&Packet::rpl(id, NickRpl::InvalidNick { reason }))?;
            return Ok(());
        }

        self.session.nick = cmd.nick.clone();
        self.tx.send(&Packet::rpl(id, NickRpl::Success))?;
        self.room.lock().await.nick(self.session.id, cmd.nick);

        Ok(())
    }

    async fn handle_send(&mut self, id: u64, cmd: SendCmd) -> anyhow::Result<()> {
        if let Some(reason) = util::check_content(&cmd.content) {
            self.tx
                .send(&Packet::rpl(id, SendRpl::InvalidContent { reason }))?;
            return Ok(());
        }

        let message = self
            .room
            .lock()
            .await
            .send(self.session.id, cmd.parent, cmd.content);

        self.tx
            .send(&Packet::rpl(id, SendRpl::Success { message }))?;

        Ok(())
    }

    async fn handle_who(&mut self, id: u64, _cmd: WhoCmd) -> anyhow::Result<()> {
        let (you, others) = self.room.lock().await.who(self.session.id);
        self.tx.send(&Packet::rpl(id, WhoRpl { you, others }))?;
        Ok(())
    }

    async fn handle_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        match packet {
            Packet::Cmd { id, cmd } => match cmd {
                Cmd::Hello(_) => Err(anyhow!("unexpected Hello cmd")),
                Cmd::Nick(cmd) => self.handle_nick(id, cmd).await,
                Cmd::Send(cmd) => self.handle_send(id, cmd).await,
                Cmd::Who(cmd) => self.handle_who(id, cmd).await,
            },
            Packet::Rpl { .. } => Err(anyhow!("unexpected rpl")),
            Packet::Ntf { .. } => Err(anyhow!("unexpected ntf")),
        }
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        while let Some(packet) = self.rx.recv().await? {
            self.handle_packet(packet).await?;
        }
        Ok(())
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

    async fn handle_hello(
        &self,
        tx: &ConnTx,
        id: u64,
        cmd: HelloCmd,
    ) -> anyhow::Result<Option<(String, Session)>> {
        if let Some(reason) = util::check_room(&cmd.room) {
            tx.send(&Packet::rpl(id, HelloRpl::InvalidRoom { reason }))?;
            return Ok(None);
        }
        if let Some(reason) = util::check_nick(&cmd.nick) {
            tx.send(&Packet::rpl(id, HelloRpl::InvalidNick { reason }))?;
            return Ok(None);
        }
        if let Some(reason) = util::check_identity(&cmd.identity) {
            tx.send(&Packet::rpl(id, HelloRpl::InvalidIdentity { reason }))?;
            return Ok(None);
        }

        let session = Session {
            id: SessionId::of(&format!("{}", rand::thread_rng().gen::<u64>())),
            nick: cmd.nick,
            identity: Identity::of(&cmd.identity),
        };

        Ok(Some((cmd.room, session)))
    }

    async fn greet(&self, tx: ConnTx, mut rx: ConnRx) -> anyhow::Result<ServerSession> {
        let (id, room, session) = loop {
            let (id, cmd) = match rx.recv().await? {
                Some(Packet::Cmd {
                    id,
                    cmd: Cmd::Hello(cmd),
                }) => (id, cmd),
                Some(_) => return Err(anyhow!("not a Hello packet")),
                None => return Err(anyhow!("connection closed during greeting")),
            };

            if let Some((room, session)) = self.handle_hello(&tx, id, cmd).await? {
                break (id, room, session);
            }
        };

        let room = self.room(room).await;

        {
            let mut room = room.lock().await;

            let you = session.clone();
            let others = room
                .clients
                .values()
                .map(|client| client.session.clone())
                .collect::<Vec<_>>();
            let last_message = room.last_message;

            tx.send(&Packet::rpl(
                id,
                HelloRpl::Success {
                    you,
                    others,
                    last_message,
                },
            ))?;

            room.join(Client {
                session: session.clone(),
                send: tx.clone(),
            });
        }

        Ok(ServerSession {
            tx,
            rx,
            room,
            session,
        })
    }
    async fn greet_and_run(&self, tx: ConnTx, rx: ConnRx) -> anyhow::Result<()> {
        let mut session = self.greet(tx, rx).await?;
        let result = session.run().await;
        session.room.lock().await.part(session.session.id);
        result
    }

    /// Wrapper for [`ConnMaintenance::perform`] so it returns an
    /// [`anyhow::Result`].
    async fn maintain(maintenance: ConnMaintenance) -> anyhow::Result<()> {
        maintenance.perform().await?;
        Ok(())
    }

    async fn handle_conn(&self, stream: TcpStream) -> anyhow::Result<()> {
        let stream = tokio_tungstenite::accept_async(stream).await?;
        let (tx, rx, maintenance) = conn::new(stream, Duration::from_secs(10))?;
        tokio::try_join!(self.greet_and_run(tx, rx), Self::maintain(maintenance))?;
        Ok(())
    }

    async fn on_conn(self, stream: TcpStream) -> anyhow::Result<()> {
        let peer_addr = stream.peer_addr()?;
        info!("<{peer_addr}> Connected");

        if let Err(e) = self.handle_conn(stream).await {
            warn!("<{peer_addr}> Err: {e}");
        }

        info!("<{peer_addr}> Disconnected");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let server = Server::new();
    let listener = TcpListener::bind(("::0", 40080)).await.unwrap();
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(server.clone().on_conn(stream));
    }
}
