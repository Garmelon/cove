use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cove_core::conn::{self, ConnMaintenance, ConnRx, ConnTx};
use cove_core::packets::{
    Cmd, IdentifyCmd, IdentifyRpl, JoinNtf, NickNtf, NickRpl, Ntf, Packet, PartNtf, RoomCmd,
    RoomRpl, Rpl, SendNtf, SendRpl, WhoRpl,
};
use cove_core::replies::Replies;
use cove_core::{replies, Session, SessionId};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, MutexGuard};

// TODO Split into "interacting" and "maintenance" parts?
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    CouldNotConnect(conn::Error),
    #[error("{0}")]
    Conn(#[from] conn::Error),
    #[error("{0}")]
    Reply(#[from] replies::Error),
    #[error("invalid room: {0}")]
    InvalidRoom(String),
    #[error("invalid identity: {0}")]
    InvalidIdentity(String),
    #[error("maintenance aborted")]
    MaintenanceAborted,
    #[error("not connected")]
    NotConnected,
    #[error("incorrect reply type")]
    IncorrectReplyType,
}

#[derive(Debug)]
pub enum Event {
    StateChanged,
    IdentificationRequired,
    // TODO Add events for joining, parting, sending, ...
}

pub struct Present {
    pub session: Session,
    pub others: HashMap<SessionId, Session>,
}

impl Present {
    fn session_map(sessions: &[Session]) -> HashMap<SessionId, Session> {
        sessions
            .iter()
            .map(|session| (session.id, session.clone()))
            .collect()
    }

    fn new(session: &Session, others: &[Session]) -> Self {
        Self {
            session: session.clone(),
            others: Self::session_map(others),
        }
    }

    fn update(&mut self, session: &Session, others: &[Session]) {
        self.session = session.clone();
        self.others = Self::session_map(others);
    }

    fn update_session(&mut self, session: &Session) {
        self.session = session.clone();
    }

    fn join(&mut self, who: Session) {
        self.others.insert(who.id, who);
    }

    fn nick(&mut self, who: Session) {
        self.others.insert(who.id, who);
    }

    fn part(&mut self, who: Session) {
        self.others.remove(&who.id);
    }
}

pub enum Status {
    ChoosingRoom,
    Identifying,
    IdRequired(Option<String>),
    Present(Present),
}

impl Status {
    fn present(&self) -> Option<&Present> {
        match self {
            Status::Present(present) => Some(present),
            Status::ChoosingRoom | Status::Identifying | Status::IdRequired(_) => None,
        }
    }

    fn present_mut(&mut self) -> Option<&mut Present> {
        match self {
            Status::Present(present) => Some(present),
            Status::ChoosingRoom | Status::Identifying | Status::IdRequired(_) => None,
        }
    }
}

pub struct Connected {
    tx: ConnTx,
    next_id: u64,
    replies: Replies<u64, Rpl>,
    status: Status,
}

impl Connected {
    fn new(tx: ConnTx, timeout: Duration) -> Self {
        Self {
            tx,
            next_id: 0,
            replies: Replies::new(timeout),
            status: Status::ChoosingRoom,
        }
    }

    pub fn present(&self) -> Option<&Present> {
        self.status.present()
    }
}

// The warning about enum variant sizes shouldn't matter since a connection will
// spend most its time in the Connected state anyways.
#[allow(clippy::large_enum_variant)]
pub enum State {
    Connecting,
    Connected(Connected),
    // TODO Include reason for stop
    Stopped,
}

impl State {
    pub fn connected(&self) -> Option<&Connected> {
        match self {
            Self::Connected(connected) => Some(connected),
            Self::Connecting | Self::Stopped => None,
        }
    }

    pub fn connected_mut(&mut self) -> Option<&mut Connected> {
        match self {
            Self::Connected(connected) => Some(connected),
            Self::Connecting | Self::Stopped => None,
        }
    }

    pub fn present(&self) -> Option<&Present> {
        self.connected()?.present()
    }
}

#[derive(Clone)]
pub struct CoveConn {
    state: Arc<Mutex<State>>,
    ev_tx: UnboundedSender<Event>,
}

impl CoveConn {
    // TODO Disallow modification via this MutexGuard
    pub async fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().await
    }

    async fn cmd<C, R>(&self, cmd: C) -> Result<R, Error>
    where
        C: Into<Cmd>,
        Rpl: TryInto<R>,
    {
        let pending_reply = {
            let mut state = self.state.lock().await;
            let mut connected = state.connected_mut().ok_or(Error::NotConnected)?;

            let id = connected.next_id;
            connected.next_id += 1;

            let pending_reply = connected.replies.wait_for(id);
            connected.tx.send(&Packet::cmd(id, cmd.into()))?;
            pending_reply
        };

        let rpl = pending_reply.get().await?;
        let rpl_value = rpl.try_into().map_err(|_| Error::IncorrectReplyType)?;
        Ok(rpl_value)
    }

    /// Attempt to identify with a nick and identity. Does nothing if the room
    /// doesn't require verification.
    ///
    /// This method is intended to be called whenever a CoveConn user suspects
    /// identification to be necessary. It has little overhead.
    pub async fn identify(&self, nick: &str, identity: &str) {
        {
            let mut state = self.state.lock().await;
            if let Some(connected) = state.connected_mut() {
                if let Status::IdRequired(_) = connected.status {
                    connected.status = Status::Identifying;
                    let _ = self.ev_tx.send(Event::StateChanged);
                } else {
                    return;
                }
            } else {
                return;
            }
        }

        let conn = self.clone();
        let nick = nick.to_string();
        let identity = identity.to_string();
        tokio::spawn(async move {
            // There's no need for a second locking block, or for us to see the
            // result of this command. CoveConnMt::run will set the connection's
            // status as appropriate.
            conn.cmd::<IdentifyCmd, IdentifyRpl>(IdentifyCmd { nick, identity })
                .await
        });
    }
}

/// Maintenance for a [`CoveConn`].
pub struct CoveConnMt {
    url: String,
    room: String,
    timeout: Duration,
    conn: CoveConn,
}

impl CoveConnMt {
    pub async fn run(self) -> Result<(), Error> {
        let (tx, rx, mt) = match Self::connect(&self.url, self.timeout).await {
            Ok(conn) => conn,
            Err(e) => {
                *self.conn.state.lock().await = State::Stopped;
                let _ = self.conn.ev_tx.send(Event::StateChanged);
                return Err(Error::CouldNotConnect(e));
            }
        };

        *self.conn.state.lock().await = State::Connected(Connected::new(tx, self.timeout));
        let _ = self.conn.ev_tx.send(Event::StateChanged);

        tokio::spawn(Self::join_room(self.conn.clone(), self.room));
        let result = tokio::select! {
            result = Self::recv(&self.conn,  rx) => result,
            _ = mt.perform() => Err(Error::MaintenanceAborted),
        };

        *self.conn.state.lock().await = State::Stopped;
        let _ = self.conn.ev_tx.send(Event::StateChanged);

        result
    }

    async fn connect(
        url: &str,
        timeout: Duration,
    ) -> Result<(ConnTx, ConnRx, ConnMaintenance), conn::Error> {
        let stream = tokio_tungstenite::connect_async(url).await?.0;
        let conn = conn::new(stream, timeout);
        Ok(conn)
    }

    async fn join_room(conn: CoveConn, name: String) -> Result<(), Error> {
        let _: RoomRpl = conn.cmd(RoomCmd { name }).await?;
        Ok(())
    }

    async fn recv(conn: &CoveConn, mut rx: ConnRx) -> Result<(), Error> {
        while let Some(packet) = rx.recv().await? {
            match packet {
                Packet::Cmd { .. } => {} // Ignore commands, the server shouldn't send any
                Packet::Rpl { id, rpl } => Self::on_rpl(conn, id, rpl).await?,
                Packet::Ntf { ntf } => Self::on_ntf(conn, ntf).await?,
            }
        }
        Ok(())
    }

    async fn on_rpl(conn: &CoveConn, id: u64, rpl: Rpl) -> Result<(), Error> {
        let mut state = conn.state.lock().await;
        let connected = match state.connected_mut() {
            Some(connected) => connected,
            None => return Ok(()),
        };

        match &rpl {
            Rpl::Room(RoomRpl::Success) => {
                connected.status = Status::IdRequired(None);
                let _ = conn.ev_tx.send(Event::IdentificationRequired);
            }
            Rpl::Room(RoomRpl::InvalidRoom { reason }) => {
                return Err(Error::InvalidRoom(reason.clone()))
            }
            Rpl::Identify(IdentifyRpl::Success { you, others, .. }) => {
                connected.status = Status::Present(Present::new(you, others));
                let _ = conn.ev_tx.send(Event::StateChanged);
            }
            Rpl::Identify(IdentifyRpl::InvalidNick { reason }) => {
                connected.status = Status::IdRequired(Some(reason.clone()));
                let _ = conn.ev_tx.send(Event::IdentificationRequired);
            }
            Rpl::Identify(IdentifyRpl::InvalidIdentity { reason }) => {
                return Err(Error::InvalidIdentity(reason.clone()))
            }
            Rpl::Nick(NickRpl::Success { you }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.update_session(you);
                    let _ = conn.ev_tx.send(Event::StateChanged);
                }
            }
            Rpl::Nick(NickRpl::InvalidNick { reason: _ }) => {}
            Rpl::Send(SendRpl::Success { message }) => {
                // TODO Add message to message store or send an event
            }
            Rpl::Send(SendRpl::InvalidContent { reason: _ }) => {}
            Rpl::Who(WhoRpl { you, others }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.update(you, others);
                    let _ = conn.ev_tx.send(Event::StateChanged);
                }
            }
        }

        connected.replies.complete(&id, rpl);

        Ok(())
    }

    async fn on_ntf(conn: &CoveConn, ntf: Ntf) -> Result<(), Error> {
        let mut state = conn.state.lock().await;
        let connected = match state.connected_mut() {
            Some(connected) => connected,
            None => return Ok(()),
        };

        match ntf {
            Ntf::Join(JoinNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.join(who);
                    let _ = conn.ev_tx.send(Event::StateChanged);
                }
            }
            Ntf::Nick(NickNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.nick(who);
                    let _ = conn.ev_tx.send(Event::StateChanged);
                }
            }
            Ntf::Part(PartNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.part(who);
                    let _ = conn.ev_tx.send(Event::StateChanged);
                }
            }
            Ntf::Send(SendNtf { message }) => {
                // TODO Add message to message store or send an event
            }
        }

        Ok(())
    }
}

pub async fn new(
    url: String,
    room: String,
    timeout: Duration,
    ev_tx: UnboundedSender<Event>,
) -> (CoveConn, CoveConnMt) {
    let conn = CoveConn {
        state: Arc::new(Mutex::new(State::Connecting)),
        ev_tx,
    };
    let mt = CoveConnMt {
        url,
        room,
        timeout,
        conn,
    };
    (mt.conn.clone(), mt)
}
