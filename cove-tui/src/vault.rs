use std::path::Path;
use std::{fs, thread};

use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

enum Request {
    Close(oneshot::Sender<()>),
    Nop,
}

pub struct Vault {
    tx: mpsc::Sender<Request>,
}

impl Vault {
    pub async fn close(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Request::Close(tx)).await;
        let _ = rx.await;
    }
}

fn run(conn: Connection, mut rx: mpsc::Receiver<Request>) -> anyhow::Result<()> {
    while let Some(request) = rx.blocking_recv() {
        match request {
            // Drops the Sender resulting in `Vault::close` exiting
            Request::Close(_) => break,
            Request::Nop => {}
        }
    }
    Ok(())
}

pub fn launch(path: &Path) -> rusqlite::Result<Vault> {
    // If this fails, rusqlite will complain about not being able to open the db
    // file, which saves me from adding a separate vault error type.
    let _ = fs::create_dir_all(path.parent().expect("path to file"));

    let conn = Connection::open(path)?;
    let (tx, rx) = mpsc::channel(8);
    thread::spawn(move || run(conn, rx));
    Ok(Vault { tx })
}
