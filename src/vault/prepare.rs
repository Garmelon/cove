use rusqlite::Connection;

pub fn prepare(conn: &mut Connection) -> rusqlite::Result<()> {
    println!("Opening vault");

    // This temporary table has no foreign key constraint on euph_rooms since
    // cross-schema constraints like that are not supported by SQLite.
    conn.execute_batch(
        "
        CREATE TEMPORARY TABLE euph_trees (
            room TEXT NOT NULL,
            id INT NOT NULL,

            PRIMARY KEY (room, id)
        ) STRICT;

        INSERT INTO euph_trees (room, id)
        SELECT room, id
        FROM euph_msgs
        WHERE parent IS NULL
        UNION
        SELECT room, parent
        FROM euph_msgs
        WHERE parent IS NOT NULL
        AND NOT EXISTS(
            SELECT *
            FROM euph_msgs AS parents
            WHERE parents.room = euph_msgs.room
            AND parents.id = euph_msgs.parent
        );
        ",
    )?;

    // Cache amount of unseen messages per room because counting them takes far
    // too long. Uses triggers to move as much of the updating logic as possible
    // into SQLite.
    conn.execute_batch(
        "
        CREATE TEMPORARY TABLE euph_unseen_counts (
            room   TEXT    NOT NULL,
            amount INTEGER NOT NULL,

            PRIMARY KEY (room)
        ) STRICT;

        -- There must be an entry for every existing room.
        INSERT INTO euph_unseen_counts (room, amount)
        SELECT room, 0
        FROM euph_rooms;

        INSERT OR REPLACE INTO euph_unseen_counts (room, amount)
        SELECT room, COUNT(*)
        FROM euph_msgs
        WHERE NOT seen
        GROUP BY room;

        CREATE TEMPORARY TRIGGER euc_insert_room
        AFTER INSERT ON euph_rooms
        BEGIN
            INSERT INTO euph_unseen_counts (room, amount)
            VALUES (new.room, 0);
        END;

        CREATE TEMPORARY TRIGGER euc_delete_room
        AFTER DELETE ON euph_rooms
        BEGIN
            DELETE FROM euph_unseen_counts
            WHERE room = old.room;
        END;

        CREATE TEMPORARY TRIGGER euc_insert_msg
        AFTER INSERT ON euph_msgs
        WHEN NOT new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = amount + 1
            WHERE room = new.room;
        END;

        CREATE TEMPORARY TRIGGER euc_update_msg
        AFTER UPDATE OF seen ON euph_msgs
        WHEN old.seen != new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = CASE WHEN new.seen THEN amount - 1 ELSE amount + 1 END
            WHERE room = new.room;
        END;

        CREATE TEMPORARY TRIGGER euc_delete_msg
        AFTER DELETE ON euph_msgs
        WHEN NOT old.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = amount - 1
            WHERE room = old.room;
        END;
        ",
    )?;

    Ok(())
}
