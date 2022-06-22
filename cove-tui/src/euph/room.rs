use std::convert::Infallible;
use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task, time};
use tokio_tungstenite::tungstenite;

use super::conn::{State, Status, WsStream};

#[derive(Debug)]
enum Event {
    Connected(SplitSink<WsStream, tungstenite::Message>),
    Disconnected,
    WsMessage(tungstenite::Message),
    DoPings,
    GetStatus(oneshot::Sender<Option<Status>>),
}

async fn run(
    canary: oneshot::Receiver<Infallible>,
    tx: mpsc::UnboundedSender<Event>,
    rx: mpsc::UnboundedReceiver<Event>,
    url: String,
) {
    let state = State::default();
    select! {
        _ = canary => (),
        _ = respond_to_events(state, rx) => (),
        _ = maintain_connection(tx, url) => (),
    }
}

async fn respond_to_events(
    mut state: State,
    mut rx: mpsc::UnboundedReceiver<Event>,
) -> anyhow::Result<()> {
    while let Some(event) = rx.recv().await {
        match event {
            Event::Connected(tx) => state.on_connected(tx),
            Event::Disconnected => state.on_disconnected(),
            Event::WsMessage(msg) => state.on_ws_message(msg)?,
            Event::DoPings => state.on_do_pings()?,
            Event::GetStatus(tx) => {
                let _ = tx.send(state.status());
            }
        }
    }
    Ok(())
}

async fn maintain_connection(tx: mpsc::UnboundedSender<Event>, url: String) -> anyhow::Result<()> {
    loop {
        // TODO Cookies
        let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
        let (ws_tx, ws_rx) = ws.split();
        tx.send(Event::Connected(ws_tx))?;
        select! {
            _ = receive_messages(&tx, ws_rx) => (),
            _ = prompt_pings(&tx) => ()
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
        tx.send(Event::WsMessage(msg?))?;
    }
    Ok(())
}

async fn prompt_pings(tx: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
    loop {
        // TODO Make ping delay configurable
        time::sleep(Duration::from_secs(10)).await;
        tx.send(Event::DoPings)?;
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

        task::spawn(run(canary_rx, event_tx.clone(), event_rx, url));

        Self {
            canary: canary_tx,
            tx: event_tx,
        }
    }

    pub async fn status(&self) -> anyhow::Result<Option<Status>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Event::GetStatus(tx))?;
        Ok(rx.await?)
    }
}
