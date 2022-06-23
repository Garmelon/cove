use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::{mpsc, oneshot};

use crate::euph::Snowflake;
use crate::store::{Msg, MsgStore, Path, Tree};

use super::Request;

#[derive(Debug, Clone)]
pub struct EuphMsg {
    id: Snowflake,
    parent: Option<Snowflake>,
    time: DateTime<Utc>,
    nick: String,
    content: String,
}

impl Msg for EuphMsg {
    type Id = Snowflake;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn parent(&self) -> Option<Self::Id> {
        self.parent
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    fn nick(&self) -> String {
        self.nick.clone()
    }

    fn content(&self) -> String {
        self.content.clone()
    }
}

impl From<EuphRequest> for Request {
    fn from(r: EuphRequest) -> Self {
        Self::Euph(r)
    }
}

pub struct EuphVault {
    pub(super) tx: mpsc::Sender<Request>,
    pub(super) room: String,
}

#[async_trait]
impl MsgStore<EuphMsg> for EuphVault {
    async fn path(&self, id: &Snowflake) -> Path<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::Path {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }

    async fn tree(&self, root: &Snowflake) -> Tree<EuphMsg> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::Tree {
            room: self.room.clone(),
            root: *root,
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }

    async fn prev_tree(&self, root: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::PrevTree {
            room: self.room.clone(),
            root: *root,
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }

    async fn next_tree(&self, root: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::NextTree {
            room: self.room.clone(),
            root: *root,
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }

    async fn first_tree(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::FirstTree {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }

    async fn last_tree(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::LastTree {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.tx.send(request.into()).await;
        rx.await.unwrap()
    }
}

pub(super) enum EuphRequest {
    Path {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Path<Snowflake>>,
    },
    Tree {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Tree<EuphMsg>>,
    },
    PrevTree {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    NextTree {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    FirstTree {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    LastTree {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
}

impl EuphRequest {
    pub(super) fn perform(self, conn: &Connection) {
        let result = match self {
            EuphRequest::Path { room, id, result } => Self::path(conn, room, id, result),
            EuphRequest::Tree { room, root, result } => Self::tree(conn, room, root, result),
            EuphRequest::PrevTree { room, root, result } => {
                Self::prev_tree(conn, room, root, result)
            }
            EuphRequest::NextTree { room, root, result } => {
                Self::next_tree(conn, room, root, result)
            }
            EuphRequest::FirstTree { room, result } => Self::first_tree(conn, room, result),
            EuphRequest::LastTree { room, result } => Self::last_tree(conn, room, result),
        };
        if let Err(e) = result {
            // If an error occurs here, the rest of the UI will likely panic and
            // crash soon. By printing this to stderr instead of logging it, we
            // can filter it out and read it later.
            // TODO Better vault error handling
            eprintln!("{e}");
        }
    }

    fn path(
        conn: &Connection,
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Path<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let path = conn
            .prepare(
                "
                WITH RECURSIVE path (room, id) = (
                    VALUES (?, ?)
                UNION
                    SELECT (room, parent)
                    FROM euph_msgs
                    JOIN path USING (room, id)
                )
                SELECT id
                FROM path
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, id.0], |row| row.get(0).map(Snowflake))?
            .collect::<rusqlite::Result<_>>()?;
        let path = Path::new(path);
        let _ = result.send(path);
        Ok(())
    }

    fn tree(
        conn: &Connection,
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Tree<EuphMsg>>,
    ) -> rusqlite::Result<()> {
        let msgs = conn
            .prepare(
                "
                WITH RECURSIVE tree (room, id) = (
                    VALUES (?, ?)
                UNION
                    SELECT (euph_msgs.room, euph_msgs.id)
                    FROM euph_msgs
                    JOIN tree
                        ON tree.room = euph_msgs.room
                        AND tree.id = euph_msgs.parent
                )
                SELECT (id, parent, time, name, content)
                FROM euph_msg
                JOIN tree USING (room, id)
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, root.0], |row| {
                Ok(EuphMsg {
                    id: Snowflake(row.get(0)?),
                    parent: row.get::<_, Option<u64>>(1)?.map(Snowflake),
                    time: row.get(2)?,
                    nick: row.get(3)?,
                    content: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let tree = Tree::new(root, msgs);
        let _ = result.send(tree);
        Ok(())
    }

    fn prev_tree(
        conn: &Connection,
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE room = ?
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(params![room, root.0], |row| row.get(0).map(Snowflake))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn next_tree(
        conn: &Connection,
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE room = ?
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(params![room, root.0], |row| row.get(0).map(Snowflake))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn first_tree(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE room = ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row([room], |row| row.get(0).map(Snowflake))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn last_tree(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE room = ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row([room], |row| row.get(0).map(Snowflake))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }
}
