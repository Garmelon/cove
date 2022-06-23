mod euph;
mod migrate;

use std::path::Path;
use std::{fs, thread};

use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use self::euph::EuphRequest;
pub use self::euph::{EuphMsg, EuphVault};

enum Request {
    Close(oneshot::Sender<()>),
    Euph(EuphRequest),
}

#[derive(Debug, Clone)]
pub struct Vault {
    tx: mpsc::UnboundedSender<Request>,
}

impl Vault {
    pub async fn close(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Request::Close(tx));
        let _ = rx.await;
    }

    pub fn euph(&self, room: String) -> EuphVault {
        EuphVault {
            tx: self.tx.clone(),
            room,
        }
    }
}

fn run(conn: Connection, mut rx: mpsc::UnboundedReceiver<Request>) {
    while let Some(request) = rx.blocking_recv() {
        match request {
            Request::Close(tx) => {
                println!("Optimizing vault");
                let _ = conn.execute_batch("PRAGMA optimize");
                // Ensure `Vault::close` exits only after the sqlite connection
                // has been closed properly.
                drop(conn);
                drop(tx);
                break;
            }
            Request::Euph(r) => r.perform(&conn),
        }
    }
}

pub fn launch(path: &Path) -> rusqlite::Result<Vault> {
    // If this fails, rusqlite will complain about not being able to open the db
    // file, which saves me from adding a separate vault error type.
    let _ = fs::create_dir_all(path.parent().expect("path to file"));

    let mut conn = Connection::open(path)?;

    // Setting locking mode before journal mode so no shared memory files
    // (*-shm) need to be created by sqlite. Apparently, setting the journal
    // mode is also enough to immediately acquire the exclusive lock even if the
    // database was already using WAL.
    // https://sqlite.org/pragma.html#pragma_locking_mode
    conn.pragma_update(None, "locking_mode", "exclusive")?;
    conn.pragma_update(None, "journal_mode", "wal")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "trusted_schema", false)?;

    migrate::migrate(&mut conn)?;

    let (tx, rx) = mpsc::unbounded_channel();
    thread::spawn(move || run(conn, rx));
    Ok(Vault { tx })
}
