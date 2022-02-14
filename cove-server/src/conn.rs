use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, io, result};

use cove_core::packets::Packet;
use futures::stream::{SplitSink, SplitStream};
use futures::StreamExt;
use log::debug;
use rand::Rng;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::WebSocketStream;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("WS error: {0}")]
    Ws(#[from] tungstenite::Error),
    #[error("MPSC error: {0}")]
    Mpsc(#[from] mpsc::error::SendError<Message>),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("client did not pong")]
    NoPong,
    #[error("illegal binary packet")]
    IllegalBinaryPacket,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Clone)]
pub struct ConnTx {
    peer_addr: SocketAddr,
    tx: UnboundedSender<Message>,
}

impl fmt::Debug for ConnTx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnTx").finish_non_exhaustive()
    }
}

impl ConnTx {
    pub fn send(&self, packet: &Packet) -> Result<()> {
        let str = serde_json::to_string(packet).expect("unserializable packet");
        // TODO Format somewhat nicer?
        debug!("<{}> ↑ {}", self.peer_addr, str.trim());
        self.tx.send(Message::Text(str))?;
        Ok(())
    }
}

pub struct ConnRx {
    peer_addr: SocketAddr,
    ws_rx: SplitStream<WebSocketStream<TcpStream>>,
    last_ping_payload: Arc<Mutex<Vec<u8>>>,
}

impl fmt::Debug for ConnRx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnRx").finish_non_exhaustive()
    }
}

impl ConnRx {
    pub async fn recv(&mut self) -> Result<Option<Packet>> {
        loop {
            let msg = match self.ws_rx.next().await {
                None => return Ok(None),
                Some(msg) => msg?,
            };

            let str = match msg {
                Message::Text(str) => str,
                Message::Pong(payload) => {
                    *self.last_ping_payload.lock().await = payload;
                    continue;
                }
                Message::Ping(_) => {
                    // Tungstenite automatically replies to pings
                    continue;
                }
                Message::Binary(_) => return Err(Error::IllegalBinaryPacket),
                Message::Close(_) => return Ok(None),
            };

            let packet = serde_json::from_str(&str)?;

            // TODO Format somewhat nicer?
            debug!("<{}> ↓ {}", self.peer_addr, str.trim());

            return Ok(Some(packet));
        }
    }
}

pub struct ConnMaintenance {
    // Shoveling packets into the WS connection
    rx: UnboundedReceiver<Message>,
    ws_tx: SplitSink<WebSocketStream<TcpStream>, Message>,
    // Pinging and ponging
    tx: UnboundedSender<Message>,
    ping_delay: Duration,
    last_ping_payload: Arc<Mutex<Vec<u8>>>,
}

impl fmt::Debug for ConnMaintenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnMaintenance").finish_non_exhaustive()
    }
}

impl ConnMaintenance {
    pub async fn perform(self) -> Result<()> {
        let result = tokio::try_join!(
            Self::shovel(self.rx, self.ws_tx),
            Self::ping_pong(self.tx, self.ping_delay, self.last_ping_payload)
        );
        result.map(|_| ())
    }

    async fn shovel(
        rx: UnboundedReceiver<Message>,
        ws_tx: SplitSink<WebSocketStream<TcpStream>, Message>,
    ) -> Result<()> {
        UnboundedReceiverStream::new(rx)
            .map(Ok)
            .forward(ws_tx)
            .await?;
        Ok(())
    }

    async fn ping_pong(
        tx: UnboundedSender<Message>,
        ping_delay: Duration,
        last_ping_payload: Arc<Mutex<Vec<u8>>>,
    ) -> Result<()> {
        let mut payload = [0u8; 8];

        rand::thread_rng().fill(&mut payload);
        tx.send(Message::Ping(payload.to_vec()))?;
        tokio::time::sleep(ping_delay).await;

        loop {
            {
                let last_payload = last_ping_payload.lock().await;
                if (&payload as &[u8]) != (&last_payload as &[u8]) {
                    return Err(Error::NoPong);
                }
            };

            rand::thread_rng().fill(&mut payload);
            tx.send(Message::Ping(payload.to_vec()))?;

            tokio::time::sleep(ping_delay).await;
        }
    }
}

pub fn new(
    stream: WebSocketStream<TcpStream>,
    ping_delay: Duration,
) -> Result<(ConnTx, ConnRx, ConnMaintenance)> {
    let peer_addr = stream.get_ref().peer_addr()?;

    let (ws_tx, ws_rx) = stream.split();
    let (tx, rx) = mpsc::unbounded_channel();
    let last_ping_payload = Arc::new(Mutex::new(vec![]));

    let conn_tx = ConnTx {
        peer_addr,
        tx: tx.clone(),
    };
    let conn_rx = ConnRx {
        peer_addr,
        ws_rx,
        last_ping_payload: last_ping_payload.clone(),
    };
    let conn_maintenance = ConnMaintenance {
        ws_tx,
        rx,
        tx,
        ping_delay,
        last_ping_payload,
    };

    Ok((conn_tx, conn_rx, conn_maintenance))
}
