mod euph;
mod migrate;
mod prepare;

use std::path::Path;
use std::{fs, thread};

use log::error;
use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use self::euph::EuphRequest;
pub use self::euph::{EuphRoomVault, EuphVault};

enum Request {
    Close(oneshot::Sender<()>),
    Gc(oneshot::Sender<()>),
    Euph(EuphRequest),
}

#[derive(Debug, Clone)]
pub struct Vault {
    tx: mpsc::UnboundedSender<Request>,
    ephemeral: bool,
}

impl Vault {
    pub fn ephemeral(&self) -> bool {
        self.ephemeral
    }

    pub async fn close(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Request::Close(tx));
        let _ = rx.await;
    }

    pub async fn gc(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Request::Gc(tx));
        let _ = rx.await;
    }

    pub fn euph(&self) -> EuphVault {
        EuphVault::new(self.clone())
    }
}

fn run(mut conn: Connection, mut rx: mpsc::UnboundedReceiver<Request>) {
    while let Some(request) = rx.blocking_recv() {
        match request {
            Request::Close(tx) => {
                eprintln!("Closing vault");
                if let Err(e) = conn.execute_batch("PRAGMA optimize") {
                    error!("{e}");
                }
                // Ensure `Vault::close` exits only after the sqlite connection
                // has been closed properly.
                drop(conn);
                drop(tx);
                break;
            }
            Request::Gc(tx) => {
                if let Err(e) = conn.execute_batch("ANALYZE; VACUUM;") {
                    error!("{e}");
                }
                drop(tx);
            }
            Request::Euph(r) => {
                if let Err(e) = r.perform(&mut conn) {
                    error!("{e}");
                }
            }
        }
    }
}

fn launch_from_connection(mut conn: Connection, ephemeral: bool) -> rusqlite::Result<Vault> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "trusted_schema", false)?;

    eprintln!("Opening vault");

    migrate::migrate(&mut conn)?;
    prepare::prepare(&mut conn)?;

    let (tx, rx) = mpsc::unbounded_channel();
    thread::spawn(move || run(conn, rx));
    Ok(Vault { tx, ephemeral })
}

pub fn launch(path: &Path) -> rusqlite::Result<Vault> {
    // If this fails, rusqlite will complain about not being able to open the db
    // file, which saves me from adding a separate vault error type.
    let _ = fs::create_dir_all(path.parent().expect("path to file"));

    let conn = Connection::open(path)?;

    // Setting locking mode before journal mode so no shared memory files
    // (*-shm) need to be created by sqlite. Apparently, setting the journal
    // mode is also enough to immediately acquire the exclusive lock even if the
    // database was already using WAL.
    // https://sqlite.org/pragma.html#pragma_locking_mode
    conn.pragma_update(None, "locking_mode", "exclusive")?;
    conn.pragma_update(None, "journal_mode", "wal")?;

    launch_from_connection(conn, false)
}

pub fn launch_in_memory() -> rusqlite::Result<Vault> {
    let conn = Connection::open_in_memory()?;
    launch_from_connection(conn, true)
}
