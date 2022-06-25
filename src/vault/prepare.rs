use rusqlite::Connection;

pub fn prepare(conn: &mut Connection) -> rusqlite::Result<()> {
    println!("Opening vault");
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
    )
}
