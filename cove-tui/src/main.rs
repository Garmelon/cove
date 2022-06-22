#![warn(clippy::use_self)]

mod chat;
mod euph;
mod log;
mod replies;
mod store;
mod ui;
mod vault;

use euph::api::{Data, Nick, Send};
use tokio::task;

use crate::euph::conn;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async("wss://euphoria.io/room/test/ws").await?;
    let (tx, mut rx) = conn::wrap(ws);
    println!("Connected!");

    while let Ok(data) = rx.recv().await {
        match data {
            Data::SnapshotEvent(_) => {
                let tx = tx.clone();
                task::spawn(async move { tx.send(Nick::new("TestBot".to_string())).await });
            }
            Data::SendEvent(p) => {
                let tx = tx.clone();
                match &p.0.content as &str {
                    "!ping" => {
                        task::spawn(async move { tx.send(Send::reply(p.0.id, "Pong!")).await });
                    }
                    "!test" => {
                        task::spawn(async move {
                            let status = tx.status().await.unwrap();
                            tx.send(Send::reply(p.0.id, format!("{status:#?}"))).await;
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    println!("Disconnected");
    Ok(())
}
