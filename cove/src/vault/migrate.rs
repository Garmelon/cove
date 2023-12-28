use rusqlite::Transaction;
use vault::Migration;

pub const MIGRATIONS: [Migration; 3] = [m1, m2, m3];

fn eprint_status(nr: usize, total: usize) {
    eprintln!("Migrating vault from {} to {} (out of {total})", nr, nr + 1);
}

fn m1(tx: &mut Transaction<'_>, nr: usize, total: usize) -> rusqlite::Result<()> {
    eprint_status(nr, total);
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
    eprint_status(nr, total);
    tx.execute_batch(
        "
        ALTER TABLE euph_msgs
        ADD COLUMN seen INTEGER NOT NULL DEFAULT TRUE;

        CREATE INDEX euph_idx_msgs_room_id_seen
        ON euph_msgs (room, id, seen);
        ",
    )
}

fn m3(tx: &mut Transaction<'_>, nr: usize, total: usize) -> rusqlite::Result<()> {
    eprint_status(nr, total);
    println!("  This migration might take quite a while.");
    println!("  Aborting it will not corrupt your vault.");

    // Rooms should be identified not just via their name but also their domain.
    // The domain should be required but there should be no default value.
    //
    // To accomplish this, we need to recreate and repopulate all euph related
    // tables because SQLite's ALTER TABLE is not powerful enough.

    eprintln!("  Preparing tables...");
    tx.execute_batch(
        "
        DROP INDEX euph_idx_msgs_room_id_parent;
        DROP INDEX euph_idx_msgs_room_parent_id;
        DROP INDEX euph_idx_msgs_room_id_seen;

        ALTER TABLE euph_rooms RENAME TO old_euph_rooms;
        ALTER TABLE euph_msgs RENAME TO old_euph_msgs;
        ALTER TABLE euph_spans RENAME TO old_euph_spans;
        ALTER TABLE euph_cookies RENAME TO old_euph_cookies;

        CREATE TABLE euph_rooms (
            domain       TEXT NOT NULL,
            room         TEXT NOT NULL,
            first_joined INT  NOT NULL,
            last_joined  INT  NOT NULL,

            PRIMARY KEY (domain, room)
        ) STRICT;

        CREATE TABLE euph_msgs (
            domain TEXT NOT NULL,
            room   TEXT NOT NULL,
            seen   INT  NOT NULL,

            -- Message
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

            PRIMARY KEY (domain, room, id),
            FOREIGN KEY (domain, room) REFERENCES euph_rooms (domain, room)
                ON DELETE CASCADE
        ) STRICT;

        CREATE TABLE euph_spans (
            domain TEXT NOT NULL,
            room   TEXT NOT NULL,
            start  INT,
            end    INT,

            UNIQUE (room, domain, start, end),
            FOREIGN KEY (domain, room) REFERENCES euph_rooms (domain, room)
                ON DELETE CASCADE,
            CHECK (start IS NULL OR end IS NOT NULL)
        ) STRICT;

        CREATE TABLE euph_cookies (
            domain TEXT NOT NULL,
            cookie TEXT NOT NULL
        ) STRICT;
        ",
    )?;

    eprintln!("  Migrating data...");
    tx.execute_batch(
        "
        INSERT INTO euph_rooms (domain, room, first_joined, last_joined)
        SELECT 'euphoria.io', room, first_joined, last_joined
        FROM old_euph_rooms;

        INSERT INTO euph_msgs (
            domain, room, seen,
            id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
            user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
        )
        SELECT
            'euphoria.io', room, seen,
            id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
            user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
        FROM old_euph_msgs;

        INSERT INTO euph_spans (domain, room, start, end)
        SELECT 'euphoria.io', room, start, end
        FROM old_euph_spans;

        INSERT INTO euph_cookies (domain, cookie)
        SELECT 'euphoria.io', cookie
        FROM old_euph_cookies;
        ",
    )?;

    eprintln!("  Recreating indices...");
    tx.execute_batch(
        "
        CREATE INDEX euph_idx_msgs_domain_room_id_parent
        ON euph_msgs (domain, room, id, parent);

        CREATE INDEX euph_idx_msgs_domain_room_parent_id
        ON euph_msgs (domain, room, parent, id);

        CREATE INDEX euph_idx_msgs_domain_room_id_seen
        ON euph_msgs (domain, room, id, seen);
        ",
    )?;

    eprintln!("  Cleaning up loose ends...");
    tx.execute_batch(
        "
        DROP TABLE old_euph_rooms;
        DROP TABLE old_euph_msgs;
        DROP TABLE old_euph_spans;
        DROP TABLE old_euph_cookies;

        ANALYZE;
        ",
    )?;

    Ok(())
}
