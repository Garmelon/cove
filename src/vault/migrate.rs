use rusqlite::Transaction;
use vault::Migration;

pub const MIGRATIONS: [Migration; 2] = [m1, m2];

fn m1(tx: &mut Transaction<'_>, nr: usize, total: usize) -> rusqlite::Result<()> {
    eprintln!("Migrating vault from {} to {} (out of {total})", nr, nr + 1);
    tx.execute_batch(
        "
        CREATE TABLE euph_rooms (
            room         TEXT NOT NULL PRIMARY KEY,
            first_joined INT  NOT NULL,
            last_joined  INT  NOT NULL
        ) STRICT;

        CREATE TABLE euph_msgs (
            -- Message
            room              TEXT NOT NULL,
            id                INT  NOT NULL,
            parent            INT,
            previous_edit_id  INT,
            time              INT  NOT NULL,
            content           TEXT NOT NULL,
            encryption_key_id TEXT,
            edited            INT,
            deleted           INT,
            truncated         INT  NOT NULL,

            -- SessionView
            user_id             TEXT NOT NULL,
            name                TEXT,
            server_id           TEXT NOT NULL,
            server_era          TEXT NOT NULL,
            session_id          TEXT NOT NULL,
            is_staff            INT  NOT NULL,
            is_manager          INT  NOT NULL,
            client_address      TEXT,
            real_client_address TEXT,

            PRIMARY KEY (room, id),
            FOREIGN KEY (room) REFERENCES euph_rooms (room)
                ON DELETE CASCADE
        ) STRICT;

        CREATE TABLE euph_spans (
            room  TEXT NOT NULL,
            start INT,
            end   INT,

            UNIQUE (room, start, end),
            FOREIGN KEY (room) REFERENCES euph_rooms (room)
                ON DELETE CASCADE,
            CHECK (start IS NULL OR end IS NOT NULL)
        ) STRICT;

        CREATE TABLE euph_cookies (
            cookie TEXT NOT NULL
        ) STRICT;

        CREATE INDEX euph_idx_msgs_room_id_parent
        ON euph_msgs (room, id, parent);

        CREATE INDEX euph_idx_msgs_room_parent_id
        ON euph_msgs (room, parent, id);
        ",
    )
}

fn m2(tx: &mut Transaction<'_>, nr: usize, total: usize) -> rusqlite::Result<()> {
    eprintln!("Migrating vault from {} to {} (out of {total})", nr, nr + 1);
    tx.execute_batch(
        "
        ALTER TABLE euph_msgs
        ADD COLUMN seen INTEGER NOT NULL DEFAULT TRUE;

        CREATE INDEX euph_idx_msgs_room_id_seen
        ON euph_msgs (room, id, seen);
        ",
    )
}
