mod euph;
mod migrate;
mod prepare;

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use tz::TimeZone;
use vault::tokio::TokioVault;
use vault::Action;

pub use self::euph::{EuphRoomVault, EuphVault, RoomIdentifier};

#[derive(Debug, Clone)]
pub struct Vault {
    tokio_vault: TokioVault,
    time_zone: TimeZone,
    ephemeral: bool,
}

struct GcAction;

impl Action for GcAction {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.execute_batch("ANALYZE; VACUUM;")
    }
}

impl Vault {
    pub fn ephemeral(&self) -> bool {
        self.ephemeral
    }

    pub async fn close(&self) {
        self.tokio_vault.stop().await;
    }

    pub async fn gc(&self) -> Result<(), vault::tokio::Error<rusqlite::Error>> {
        self.tokio_vault.execute(GcAction).await
    }

    pub fn euph(&self) -> EuphVault {
        EuphVault::new(self.clone())
    }
}

fn launch_from_connection(
    conn: Connection,
    time_zone: TimeZone,
    ephemeral: bool,
) -> rusqlite::Result<Vault> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "trusted_schema", false)?;

    let tokio_vault = TokioVault::launch_and_prepare(conn, &migrate::MIGRATIONS, prepare::prepare)?;
    Ok(Vault {
        tokio_vault,
        time_zone,
        ephemeral,
    })
}

pub fn launch(path: &Path, time_zone: TimeZone) -> rusqlite::Result<Vault> {
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

    launch_from_connection(conn, time_zone, false)
}

pub fn launch_in_memory(time_zone: TimeZone) -> rusqlite::Result<Vault> {
    let conn = Connection::open_in_memory()?;
    launch_from_connection(conn, time_zone, true)
}
