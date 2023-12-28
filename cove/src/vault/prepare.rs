use rusqlite::Connection;

pub fn prepare(conn: &mut Connection) -> rusqlite::Result<()> {
    eprintln!("Preparing vault");

    // Cache ids of tree roots.
    conn.execute_batch(
        "
        CREATE TEMPORARY TABLE euph_trees (
            domain TEXT NOT NULL,
            room TEXT NOT NULL,
            id INT NOT NULL,

            PRIMARY KEY (domain, room, id)
        ) STRICT;

        INSERT INTO euph_trees (domain, room, id)
        SELECT domain, room, id
        FROM euph_msgs
        WHERE parent IS NULL
        UNION
        SELECT domain, room, parent
        FROM euph_msgs
        WHERE parent IS NOT NULL
        AND NOT EXISTS(
            SELECT *
            FROM euph_msgs AS parents
            WHERE parents.domain = euph_msgs.domain
            AND parents.room = euph_msgs.room
            AND parents.id = euph_msgs.parent
        );

        CREATE TEMPORARY TRIGGER et_delete_room
        AFTER DELETE ON main.euph_rooms
        BEGIN
            DELETE FROM euph_trees
            WHERE domain = old.domain
            AND room = old.room;
        END;

        CREATE TEMPORARY TRIGGER et_insert_msg_without_parent
        AFTER INSERT ON main.euph_msgs
        WHEN new.parent IS NULL
        BEGIN
            INSERT OR IGNORE INTO euph_trees (domain, room, id)
            VALUES (new.domain, new.room, new.id);
        END;

        CREATE TEMPORARY TRIGGER et_insert_msg_with_parent
        AFTER INSERT ON main.euph_msgs
        WHEN new.parent IS NOT NULL
        BEGIN
            DELETE FROM euph_trees
            WHERE domain = new.domain
            AND room = new.room
            AND id = new.id;

            INSERT OR IGNORE INTO euph_trees (domain, room, id)
            SELECT *
            FROM (VALUES (new.domain, new.room, new.parent))
            WHERE NOT EXISTS(
                SELECT *
                FROM euph_msgs
                WHERE domain = new.domain
                AND room = new.room
                AND id = new.parent
                AND parent IS NOT NULL
            );
        END;
        ",
    )?;

    // Cache amount of unseen messages per room.
    conn.execute_batch(
        "
        CREATE TEMPORARY TABLE euph_unseen_counts (
            domain TEXT    NOT NULL,
            room   TEXT    NOT NULL,
            amount INTEGER NOT NULL,

            PRIMARY KEY (domain, room)
        ) STRICT;

        -- There must be an entry for every existing room.
        INSERT INTO euph_unseen_counts (domain, room, amount)
        SELECT domain, room, 0
        FROM euph_rooms;

        INSERT OR REPLACE INTO euph_unseen_counts (domain, room, amount)
        SELECT domain, room, COUNT(*)
        FROM euph_msgs
        WHERE NOT seen
        GROUP BY domain, room;

        CREATE TEMPORARY TRIGGER euc_insert_room
        AFTER INSERT ON main.euph_rooms
        BEGIN
            INSERT INTO euph_unseen_counts (domain, room, amount)
            VALUES (new.domain, new.room, 0);
        END;

        CREATE TEMPORARY TRIGGER euc_delete_room
        AFTER DELETE ON main.euph_rooms
        BEGIN
            DELETE FROM euph_unseen_counts
            WHERE domain = old.domain
            AND room = old.room;
        END;

        CREATE TEMPORARY TRIGGER euc_insert_msg
        AFTER INSERT ON main.euph_msgs
        WHEN NOT new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = amount + 1
            WHERE domain = new.domain
            AND room = new.room;
        END;

        CREATE TEMPORARY TRIGGER euc_update_msg
        AFTER UPDATE OF seen ON main.euph_msgs
        WHEN old.seen != new.seen
        BEGIN
            UPDATE euph_unseen_counts
            SET amount = CASE WHEN new.seen THEN amount - 1 ELSE amount + 1 END
            WHERE domain = new.domain
            AND room = new.room;
        END;
        ",
    )?;

    Ok(())
}
