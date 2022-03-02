use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cove_core::conn::{self, ConnMaintenance, ConnRx, ConnTx};
use cove_core::packets::{
    IdentifyRpl, JoinNtf, NickNtf, NickRpl, Ntf, Packet, PartNtf, RoomRpl, Rpl, SendNtf, SendRpl,
    WhoRpl,
};
use cove_core::{Session, SessionId};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

use crate::replies::Replies;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Conn(#[from] conn::Error),
    #[error("invalid room: {0}")]
    InvalidRoom(String),
    #[error("invalid identity: {0}")]
    InvalidIdentity(String),
    #[error("maintenance aborted")]
    MaintenanceAborted,
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
    replies: Replies<u64, Rpl>,
    status: Status,
}

impl Connected {
    fn new(tx: ConnTx, timeout: Duration) -> Self {
        Self {
            tx,
            replies: Replies::new(timeout),
            status: Status::ChoosingRoom,
        }
    }
}

pub enum CoveConn {
    Connecting,
    Connected(Connected),
    Stopped,
}

impl CoveConn {
    fn connected(&self) -> Option<&Connected> {
        match self {
            CoveConn::Connected(connected) => Some(connected),
            CoveConn::Connecting | CoveConn::Stopped => None,
        }
    }

    fn connected_mut(&mut self) -> Option<&mut Connected> {
        match self {
            CoveConn::Connected(connected) => Some(connected),
            CoveConn::Connecting | CoveConn::Stopped => None,
        }
    }
}

/// Maintenance for a [`CoveConn`].
pub struct CoveConnMt {
    url: String,
    timeout: Duration,
    conn: Arc<Mutex<CoveConn>>,
    ev_tx: UnboundedSender<Event>,
}

impl CoveConnMt {
    pub async fn run(self) -> Result<(), Error> {
        let (tx, rx, mt) = match Self::connect(&self.url, self.timeout).await {
            Ok(conn) => conn,
            Err(e) => {
                *self.conn.lock().await = CoveConn::Stopped;
                return Err(Error::Conn(e));
            }
        };

        *self.conn.lock().await = CoveConn::Connected(Connected::new(tx, self.timeout));
        self.ev_tx.send(Event::StateChanged);

        let result = tokio::select! {
            result = Self::recv(&self.conn, &self.ev_tx, rx) => result,
            _ = mt.perform() => Err(Error::MaintenanceAborted),
        };

        *self.conn.lock().await = CoveConn::Stopped;
        self.ev_tx.send(Event::StateChanged);

        result
    }

    async fn connect(
        url: &str,
        timeout: Duration,
    ) -> Result<(ConnTx, ConnRx, ConnMaintenance), conn::Error> {
        let stream = tokio_tungstenite::connect_async(url).await?.0;
        let conn = conn::new(stream, timeout)?;
        Ok(conn)
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
            Rpl::Room(RoomRpl::Success) => {}
            Rpl::Room(RoomRpl::InvalidRoom { reason }) => {
                return Err(Error::InvalidRoom(reason.clone()))
            }
            Rpl::Identify(IdentifyRpl::Success { you, others, .. }) => {
                connected.status = Status::Present(Present::new(you, others));
                ev_tx.send(Event::StateChanged);
            }
            Rpl::Identify(IdentifyRpl::InvalidNick { reason }) => {}
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
    timeout: Duration,
) -> (Arc<Mutex<CoveConn>>, CoveConnMt, UnboundedReceiver<Event>) {
    let conn = Arc::new(Mutex::new(CoveConn::Connecting));
    let (ev_tx, ev_rx) = mpsc::unbounded_channel();
    let mt = CoveConnMt {
        url,
        timeout,
        conn,
        ev_tx,
    };
    (mt.conn.clone(), mt, ev_rx)
}
