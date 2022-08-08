use rusqlite::Connection;

pub fn prepare(conn: &mut Connection) -> rusqlite::Result<()> {
    println!("Opening vault");

    // Cache ids of tree roots.
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

        CREATE TEMPORARY TRIGGER et_delete_room
        AFTER DELETE ON main.euph_rooms
        BEGIN
            DELETE FROM euph_trees
            WHERE room = old.room;
        END;

        CREATE TEMPORARY TRIGGER et_insert_msg_without_parent
        AFTER INSERT ON main.euph_msgs
        WHEN new.parent IS NULL
        BEGIN
            INSERT OR IGNORE INTO euph_trees (room, id)
            VALUES (new.room, new.id);
        END;

        CREATE TEMPORARY TRIGGER et_insert_msg_with_parent
        AFTER INSERT ON main.euph_msgs
        WHEN new.parent IS NOT NULL
        BEGIN
            DELETE FROM euph_trees
            WHERE room = new.room
            AND id = new.id;

            INSERT OR IGNORE INTO euph_trees (room, id)
            SELECT *
            FROM (VALUES (new.room, new.parent))
            WHERE NOT EXISTS(
                SELECT *
                FROM euph_msgs
                WHERE room = new.room
                AND id = new.parent
                AND parent IS NOT NULL
            );
        END;
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
        AFTER INSERT ON main.euph_rooms
        BEGIN
            INSERT INTO euph_unseen_counts (room, amount)
            VALUES (new.room, 0);
        END;

        CREATE TEMPORARY TRIGGER euc_delete_room
        AFTER DELETE ON main.euph_rooms
        BEGIN
            DELETE FROM euph_unseen_counts
            WHERE room = old.room;
        END;

        CREATE TEMPORARY TRIGGER euc_insert_msg
        AFTER INSERT ON main.euph_msgs
        WHEN NOT new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = amount + 1
            WHERE room = new.room;
        END;

        CREATE TEMPORARY TRIGGER euc_update_msg
        AFTER UPDATE OF seen ON main.euph_msgs
        WHEN old.seen != new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = CASE WHEN new.seen THEN amount - 1 ELSE amount + 1 END
            WHERE room = new.room;
        END;

        CREATE TEMPORARY TRIGGER euc_delete_msg
        AFTER DELETE ON main.euph_msgs
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
