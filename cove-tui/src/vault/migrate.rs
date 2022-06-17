use rusqlite::{Connection, Transaction};

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let mut tx = conn.transaction()?;

    let user_version: usize =
        tx.query_row("SELECT * FROM pragma_user_version", [], |r| r.get(0))?;

    let total = MIGRATIONS.len();
    for (i, migration) in MIGRATIONS.iter().enumerate().skip(user_version) {
        println!("Migrating vault from {} to {} (out of {})", i, i + 1, total);
        migration(&mut tx)?;
    }

    tx.pragma_update(None, "user_version", total)?;
    tx.commit()
}

const MIGRATIONS: [fn(&mut Transaction) -> rusqlite::Result<()>; 1] = [m1];

fn m1(tx: &mut Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE test (
            foo TEXT
        );
        ",
    )
}
