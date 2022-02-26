use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cove_core::conn::{self, ConnMaintenance, ConnRx, ConnTx};
use cove_core::packets::{
    Cmd, IdentifyCmd, IdentifyRpl, NickRpl, Ntf, Packet, RoomRpl, Rpl, SendRpl, WhoRpl,
};
use cove_core::{Session, SessionId};
use tokio::sync::oneshot::{self, Sender};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::never::Never;
use crate::replies::{self, Replies};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not connected")]
    NotConnected,
    #[error("not present")]
    NotPresent,
    #[error("incorrect reply type")]
    IncorrectReplyType,
    #[error("{0}")]
    Conn(#[from] conn::Error),
    #[error("{0}")]
    Replies(#[from] replies::Error),
}

/// State for when a websocket connection exists.
struct Connected {
    tx: ConnTx,
    next_id: u64,
    replies: Replies<u64, Rpl>,
}

/// State for when a client has fully joined a room.
pub struct Present {
    pub session: Session,
    pub others: HashMap<SessionId, Session>,
}

enum Status {
    /// No action required by the UI.
    Nominal,
    /// User must enter a nick.
    NickRequired,
    /// Identifying to the server. No action required by the UI.
    Identifying,
    CouldNotConnect,
    InvalidRoom(String),
    InvalidNick(String),
    InvalidIdentity(String),
    InvalidContent(String),
}

pub struct Room {
    name: String,
    identity: String,
    initial_nick: Option<String>,
    status: Status,
    connected: Option<Connected>,
    present: Option<Present>,
    still_alive: Sender<Never>,
}

impl Room {
    pub async fn new(
        name: String,
        identity: String,
        initial_nick: Option<String>,
        config: &'static Config,
    ) -> Arc<Mutex<Self>> {
        let (tx, rx) = oneshot::channel();

        let room = Arc::new(Mutex::new(Self {
            name,
            identity,
            initial_nick,
            status: Status::Nominal,
            connected: None,
            present: None,
            still_alive: tx,
        }));

        let room_clone = room.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = rx => {}
                _ = Self::bg_task(room_clone, config) => {}
            }
        });

        room
    }

    pub fn present(&self) -> Option<&Present> {
        self.present.as_ref()
    }

    async fn bg_task(room: Arc<Mutex<Room>>, config: &'static Config) {
        let mut room_verified = false;
        loop {
            if let Ok((tx, rx, mt)) = Self::connect(&config.cove_url, config.timeout).await {
                {
                    let mut room = room.lock().await;
                    room.status = Status::Nominal;
                    room.connected = Some(Connected {
                        tx,
                        next_id: 0,
                        replies: Replies::new(config.timeout),
                    });
                }

                tokio::select! {
                    _ = mt.perform() => {}
                    _ = Self::receive(room.clone(), rx, &mut room_verified) => {}
                }
            }

            if !room_verified {
                room.lock().await.status = Status::CouldNotConnect;
                return;
            }
        }
    }

    async fn connect(
        url: &str,
        timeout: Duration,
    ) -> anyhow::Result<(ConnTx, ConnRx, ConnMaintenance)> {
        let stream = tokio_tungstenite::connect_async(url).await?.0;
        let conn = conn::new(stream, timeout)?;
        Ok(conn)
    }

    async fn receive(
        room: Arc<Mutex<Room>>,
        mut rx: ConnRx,
        room_verified: &mut bool,
    ) -> anyhow::Result<()> {
        while let Some(packet) = rx.recv().await? {
            match packet {
                Packet::Cmd { .. } => {} // Ignore, the server never sends commands
                Packet::Rpl { id, rpl } => {
                    room.lock().await.on_rpl(&room, id, rpl, room_verified)?;
                }
                Packet::Ntf { ntf } => room.lock().await.on_ntf(ntf),
            }
        }
        Ok(())
    }

    fn on_rpl(
        &mut self,
        room: &Arc<Mutex<Room>>,
        id: u64,
        rpl: Rpl,
        room_verified: &mut bool,
    ) -> anyhow::Result<()> {
        match &rpl {
            Rpl::Room(RoomRpl::Success) => {
                *room_verified = true;
                if let Some(nick) = &self.initial_nick {
                    // TODO Use previous nick if there is one
                    tokio::spawn(Self::identify(
                        room.clone(),
                        nick.clone(),
                        self.identity.clone(),
                    ));
                } else {
                    self.status = Status::NickRequired;
                }
            }
            Rpl::Room(RoomRpl::InvalidRoom { reason }) => {
                self.status = Status::InvalidRoom(reason.clone());
                anyhow::bail!("invalid room");
            }
            Rpl::Identify(IdentifyRpl::Success {
                you,
                others,
                last_message,
            }) => {
                let others = others
                    .iter()
                    .map(|session| (session.id, session.clone()))
                    .collect();
                self.present = Some(Present {
                    session: you.clone(),
                    others,
                });
                // TODO Send last message to store
            }
            Rpl::Identify(IdentifyRpl::InvalidNick { reason }) => {
                self.status = Status::InvalidNick(reason.clone());
            }
            Rpl::Identify(IdentifyRpl::InvalidIdentity { reason }) => {
                self.status = Status::InvalidIdentity(reason.clone());
            }
            Rpl::Nick(NickRpl::Success { you }) => {
                if let Some(present) = &mut self.present {
                    present.session = you.clone();
                }
            }
            Rpl::Nick(NickRpl::InvalidNick { reason }) => {
                self.status = Status::InvalidNick(reason.clone());
            }
            Rpl::Send(SendRpl::Success { message }) => {
                // TODO Send message to store
            }
            Rpl::Send(SendRpl::InvalidContent { reason }) => {
                self.status = Status::InvalidContent(reason.clone());
            }
            Rpl::Who(WhoRpl { you, others }) => {
                if let Some(present) = &mut self.present {
                    present.session = you.clone();
                    present.others = others
                        .iter()
                        .map(|session| (session.id, session.clone()))
                        .collect();
                }
            }
        }

        if let Some(connected) = &mut self.connected {
            connected.replies.complete(&id, rpl);
        }

        Ok(())
    }

    fn on_ntf(&mut self, ntf: Ntf) {
        match ntf {
            Ntf::Join(join) => {
                if let Some(present) = &mut self.present {
                    present.others.insert(join.who.id, join.who);
                }
            }
            Ntf::Nick(nick) => {
                if let Some(present) = &mut self.present {
                    present.others.insert(nick.who.id, nick.who);
                }
            }
            Ntf::Part(part) => {
                if let Some(present) = &mut self.present {
                    present.others.remove(&part.who.id);
                }
            }
            Ntf::Send(_) => {
                // TODO Send message to store
            }
        }
    }

    async fn cmd<C, R>(room: &Mutex<Room>, cmd: C) -> Result<R, Error>
    where
        C: Into<Cmd>,
        Rpl: TryInto<R>,
    {
        let token = {
            let mut room = room.lock().await;
            let connected = room.connected.as_mut().ok_or(Error::NotConnected)?;

            let id = connected.next_id;
            connected.next_id += 1;

            let pending_reply = connected.replies.wait_for(id);
            connected.tx.send(&Packet::cmd(id, cmd.into()))?;
            pending_reply
        };

        let rpl = token.get().await?;
        let rpl = rpl.try_into().map_err(|_| Error::IncorrectReplyType)?;
        Ok(rpl)
    }

    async fn identify(room: Arc<Mutex<Room>>, nick: String, identity: String) -> Result<(), Error> {
        let result: IdentifyRpl = Self::cmd(&room, IdentifyCmd { nick, identity }).await?;
        Ok(())
    }
}
