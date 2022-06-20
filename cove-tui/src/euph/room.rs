use std::convert::Infallible;
use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task, time};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
enum Event {
    Connected(SplitSink<WsStream, tungstenite::Message>),
    Disconnected,
    Message(tungstenite::Message),
    Ping,
}

#[derive(Debug)]
struct Connected {
    tx: SplitSink<WsStream, tungstenite::Message>,
}

#[derive(Debug)]
enum State {
    Connecting,
    Connected(Connected),
}

impl State {
    async fn run(
        self,
        canary: oneshot::Receiver<Infallible>,
        tx: mpsc::UnboundedSender<Event>,
        rx: mpsc::UnboundedReceiver<Event>,
        url: String,
    ) {
        select! {
            _ = canary => (),
            _ = Self::maintain_connection(tx, url) => (),
            _ = self.respond_to_events(rx) => (),
        }
    }

    async fn maintain_connection(
        tx: mpsc::UnboundedSender<Event>,
        url: String,
    ) -> anyhow::Result<()> {
        loop {
            // TODO Cookies
            let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
            let (ws_tx, ws_rx) = ws.split();
            tx.send(Event::Connected(ws_tx))?;
            select! {
                _ = Self::receive_messages(&tx, ws_rx) => (),
                _ = Self::prompt_pings(&tx) => ()
            }
            tx.send(Event::Disconnected)?;
            // TODO Make reconnect delay configurable
            time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn receive_messages(
        tx: &mpsc::UnboundedSender<Event>,
        mut rx: SplitStream<WsStream>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.next().await {
            tx.send(Event::Message(msg?))?;
        }
        Ok(())
    }

    async fn prompt_pings(tx: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
        loop {
            // TODO Make ping delay configurable
            time::sleep(Duration::from_secs(10)).await;
            tx.send(Event::Ping)?;
        }
    }

    async fn respond_to_events(mut self, mut rx: mpsc::UnboundedReceiver<Event>) {
        while let Some(event) = rx.recv().await {
            match event {
                Event::Connected(tx) => self = State::Connected(Connected { tx }),
                Event::Disconnected => self = State::Connecting,
                Event::Message(_) => todo!(),
                Event::Ping => todo!(),
            }
        }
    }
}

pub struct Room {
    canary: oneshot::Sender<Infallible>,
    tx: mpsc::UnboundedSender<Event>,
}

impl Room {
    pub fn start(url: String) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (canary_tx, canary_rx) = oneshot::channel();

        task::spawn(State::Connecting.run(canary_rx, event_tx.clone(), event_rx, url));

        Self {
            canary: canary_tx,
            tx: event_tx,
        }
    }
}
