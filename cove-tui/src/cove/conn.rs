use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cove_core::conn::{self, ConnMaintenance, ConnRx, ConnTx};
use cove_core::packets::{
    Cmd, IdentifyCmd, IdentifyRpl, JoinNtf, NickNtf, NickRpl, Ntf, Packet, PartNtf, RoomCmd,
    RoomRpl, Rpl, SendNtf, SendRpl, WhoRpl,
};
use cove_core::{Session, SessionId};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

use crate::replies::{self, Replies};

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

pub enum Event {
    StateChanged,
    // TODO Add events for joining, parting, sending, ...
}

pub struct Present {
    session: Session,
    others: HashMap<SessionId, Session>,
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

    fn join(&self, who: Session) {
        self.others.insert(who.id, who);
    }

    fn nick(&self, who: Session) {
        self.others.insert(who.id, who);
    }

    fn part(&self, who: Session) {
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
}

pub enum State {
    Connecting,
    Connected(Connected),
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
}

pub struct CoveConn {
    state: State,
    ev_tx: UnboundedSender<Event>,
}

impl CoveConn {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    pub fn connected(&self) -> Option<&Connected> {
        self.state.connected()
    }

    pub fn connected_mut(&mut self) -> Option<&mut Connected> {
        self.state.connected_mut()
    }

    async fn cmd<C, R>(conn: &Mutex<Self>, cmd: C) -> Result<R, Error>
    where
        C: Into<Cmd>,
        Rpl: TryInto<R>,
    {
        let pending_reply = {
            let mut conn = conn.lock().await;
            let mut connected = conn.connected_mut().ok_or(Error::NotConnected)?;

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
    pub async fn identify(conn: Arc<Mutex<Self>>, nick: &str, identity: &str) {
        {
            let mut conn = conn.lock().await;
            if let Some(connected) = conn.connected_mut() {
                if let Status::IdRequired(_) = connected.status {
                    connected.status = Status::Identifying;
                    conn.ev_tx.send(Event::StateChanged);
                } else {
                    return;
                }
            } else {
                return;
            }
        }

        let nick = nick.to_string();
        let identity = identity.to_string();
        tokio::spawn(async move {
            // There's no need for a second locking block, or for us to see the
            // result of this command. CoveConnMt::run will set the connection's
            // status as appropriate.
            Self::cmd::<IdentifyCmd, IdentifyRpl>(&conn, IdentifyCmd { nick, identity }).await
        });
    }
}

/// Maintenance for a [`CoveConn`].
pub struct CoveConnMt {
    url: String,
    room: String,
    timeout: Duration,
    conn: Arc<Mutex<CoveConn>>,
    ev_tx: UnboundedSender<Event>,
}

impl CoveConnMt {
    pub async fn run(self) -> Result<(), Error> {
        let (tx, rx, mt) = match Self::connect(&self.url, self.timeout).await {
            Ok(conn) => conn,
            Err(e) => {
                *self.conn.lock().await.state_mut() = State::Stopped;
                self.ev_tx.send(Event::StateChanged);
                return Err(Error::CouldNotConnect(e));
            }
        };

        *self.conn.lock().await.state_mut() = State::Connected(Connected::new(tx, self.timeout));
        self.ev_tx.send(Event::StateChanged);

        tokio::spawn(Self::join_room(self.conn.clone(), self.room));
        let result = tokio::select! {
            result = Self::recv(&self.conn, &self.ev_tx, rx) => result,
            _ = mt.perform() => Err(Error::MaintenanceAborted),
        };

        *self.conn.lock().await.state_mut() = State::Stopped;
        self.ev_tx.send(Event::StateChanged);

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

    async fn join_room(conn: Arc<Mutex<CoveConn>>, name: String) -> Result<(), Error> {
        let reply: RoomRpl = CoveConn::cmd(&conn, RoomCmd { name }).await?;
        Ok(())
    }

    async fn recv(
        conn: &Mutex<CoveConn>,
        ev_tx: &UnboundedSender<Event>,
        mut rx: ConnRx,
    ) -> Result<(), Error> {
        while let Some(packet) = rx.recv().await? {
            match packet {
                Packet::Cmd { id, cmd } => {
                    // Ignore commands as the server doesn't send them.
                }
                Packet::Rpl { id, rpl } => Self::on_rpl(&conn, &ev_tx, id, rpl).await?,
                Packet::Ntf { ntf } => Self::on_ntf(&conn, &ev_tx, ntf).await?,
            }
        }
        Ok(())
    }

    async fn on_rpl(
        conn: &Mutex<CoveConn>,
        ev_tx: &UnboundedSender<Event>,
        id: u64,
        rpl: Rpl,
    ) -> Result<(), Error> {
        let mut conn = conn.lock().await;
        let connected = match conn.connected_mut() {
            Some(connected) => connected,
            None => return Ok(()),
        };

        match &rpl {
            Rpl::Room(RoomRpl::Success) => {
                connected.status = Status::IdRequired(None);
                ev_tx.send(Event::StateChanged);
            }
            Rpl::Room(RoomRpl::InvalidRoom { reason }) => {
                return Err(Error::InvalidRoom(reason.clone()))
            }
            Rpl::Identify(IdentifyRpl::Success { you, others, .. }) => {
                connected.status = Status::Present(Present::new(you, others));
                ev_tx.send(Event::StateChanged);
            }
            Rpl::Identify(IdentifyRpl::InvalidNick { reason }) => {
                connected.status = Status::IdRequired(Some(reason.clone()));
                ev_tx.send(Event::StateChanged);
            }
            Rpl::Identify(IdentifyRpl::InvalidIdentity { reason }) => {
                return Err(Error::InvalidIdentity(reason.clone()))
            }
            Rpl::Nick(NickRpl::Success { you }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.update_session(you);
                    ev_tx.send(Event::StateChanged);
                }
            }
            Rpl::Nick(NickRpl::InvalidNick { reason }) => {}
            Rpl::Send(SendRpl::Success { message }) => {
                // TODO Add message to message store or send an event
            }
            Rpl::Send(SendRpl::InvalidContent { reason }) => {}
            Rpl::Who(WhoRpl { you, others }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.update(you, others);
                    ev_tx.send(Event::StateChanged);
                }
            }
        }

        connected.replies.complete(&id, rpl);

        Ok(())
    }

    async fn on_ntf(
        conn: &Mutex<CoveConn>,
        ev_tx: &UnboundedSender<Event>,
        ntf: Ntf,
    ) -> Result<(), Error> {
        let mut conn = conn.lock().await;
        let connected = match conn.connected_mut() {
            Some(connected) => connected,
            None => return Ok(()),
        };

        match ntf {
            Ntf::Join(JoinNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.join(who);
                    ev_tx.send(Event::StateChanged);
                }
            }
            Ntf::Nick(NickNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.nick(who);
                    ev_tx.send(Event::StateChanged);
                }
            }
            Ntf::Part(PartNtf { who }) => {
                if let Some(present) = connected.status.present_mut() {
                    present.part(who);
                    ev_tx.send(Event::StateChanged);
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
) -> (Arc<Mutex<CoveConn>>, CoveConnMt, UnboundedReceiver<Event>) {
    let (ev_tx, ev_rx) = mpsc::unbounded_channel();
    let conn = Arc::new(Mutex::new(CoveConn {
        state: State::Connecting,
        ev_tx: ev_tx.clone(),
    }));
    let mt = CoveConnMt {
        url,
        room,
        timeout,
        conn,
        ev_tx,
    };
    (mt.conn.clone(), mt, ev_rx)
}
