use std::mem;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{params, Connection, OptionalExtension, ToSql, Transaction};
use tokio::sync::{mpsc, oneshot};

use crate::euph::api::{Message, Snowflake, Time};
use crate::store::{Msg, MsgStore, Path, Tree};

use super::Request;

impl ToSql for Snowflake {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for Snowflake {
    fn column_result(value: ValueRef<'_>) -> Result<Self, FromSqlError> {
        u64::column_result(value).map(Self)
    }
}

impl ToSql for Time {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let timestamp = self.0.timestamp();
        Ok(ToSqlOutput::Owned(Value::Integer(timestamp)))
    }
}

impl FromSql for Time {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let timestamp = i64::column_result(value)?;
        Ok(Self(Utc.timestamp(timestamp, 0)))
    }
}

#[derive(Debug, Clone)]
pub struct EuphMsg {
    id: Snowflake,
    parent: Option<Snowflake>,
    time: Time,
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
        self.time.0
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

#[derive(Debug, Clone)]
pub struct EuphVault {
    pub(super) tx: mpsc::UnboundedSender<Request>,
    pub(super) room: String,
}

impl EuphVault {
    pub fn add_message(&self, msg: Message, prev_msg: Option<Snowflake>) {
        let request = EuphRequest::AddMsg {
            room: self.room.clone(),
            msg,
            prev_msg,
        };
        let _ = self.tx.send(request.into());
    }

    pub fn add_messages(&self, msgs: Vec<Message>, next_msg: Option<Snowflake>) {
        let request = EuphRequest::AddMsgs {
            room: self.room.clone(),
            msgs,
            next_msg,
        };
        let _ = self.tx.send(request.into());
    }
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
        let _ = self.tx.send(request.into());
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
        let _ = self.tx.send(request.into());
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
        let _ = self.tx.send(request.into());
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
        let _ = self.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn first_tree(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::FirstTree {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn last_tree(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::LastTree {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.tx.send(request.into());
        rx.await.unwrap()
    }
}

pub(super) enum EuphRequest {
    AddMsg {
        room: String,
        msg: Message,
        prev_msg: Option<Snowflake>,
    },
    AddMsgs {
        room: String,
        msgs: Vec<Message>,
        next_msg: Option<Snowflake>,
    },
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
    pub(super) fn perform(self, conn: &mut Connection) {
        let result = match self {
            EuphRequest::AddMsg {
                room,
                msg,
                prev_msg,
            } => Self::add_msg(conn, room, msg, prev_msg),
            EuphRequest::AddMsgs {
                room,
                msgs,
                next_msg,
            } => Self::add_msgs(conn, room, msgs, next_msg),
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

    fn add_span(
        tx: &Transaction,
        room: &str,
        first_msg_id: Option<Snowflake>,
        last_msg_id: Option<Snowflake>,
    ) -> rusqlite::Result<()> {
        // Retrieve all spans for the room
        let mut spans = tx
            .prepare(
                "
                SELECT start, end
                FROM euph_spans
                WHERE room = ?
                ",
            )?
            .query_map([room], |row| {
                let start = row.get::<_, Option<Snowflake>>(0)?;
                let end = row.get::<_, Option<Snowflake>>(1)?;
                Ok((start, end))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Add new span and sort spans lexicographically
        spans.push((first_msg_id, last_msg_id));
        spans.sort_unstable();

        // Combine overlapping spans (including newly added span)
        let mut cur_span: Option<(Option<Snowflake>, Option<Snowflake>)> = None;
        let mut result = vec![];
        for mut span in spans {
            if let Some(cur_span) = &mut cur_span {
                if span.0 <= cur_span.1 {
                    // Since spans are sorted lexicographically, we know that
                    // cur_span.0 <= span.0, which means that span starts inside
                    // of cur_span.
                    cur_span.1 = cur_span.1.max(span.1);
                } else {
                    // Since span doesn't overlap cur_span, we know that no
                    // later span will overlap cur_span either. The size of
                    // cur_span is thus final.
                    mem::swap(cur_span, &mut span);
                    result.push(span);
                }
            } else {
                cur_span = Some(span);
            }
        }
        if let Some(cur_span) = cur_span {
            result.push(cur_span);
        }

        // Delete all spans for the room
        tx.execute(
            "
            DELETE FROM euph_spans
            WHERE room = ?
            ",
            [room],
        )?;

        // Re-insert combined spans for the room
        let mut stmt = tx.prepare(
            "
            INSERT INTO euph_spans (room, start, end)
            VALUES (?, ?, ?)
            ",
        )?;
        for (start, end) in result {
            stmt.execute(params![room, start, end])?;
        }

        Ok(())
    }

    fn add_msg(
        conn: &Connection,
        room: String,
        msg: Message,
        prev_msg: Option<Snowflake>,
    ) -> rusqlite::Result<()> {
        todo!()
    }

    fn add_msgs(
        conn: &mut Connection,
        room: String,
        msgs: Vec<Message>,
        next_msg_id: Option<Snowflake>,
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        if msgs.is_empty() {
            Self::add_span(&tx, &room, None, next_msg_id)?;
        } else {
            let first_msg_id = msgs.first().unwrap().id;
            let last_msg_id = msgs.first().unwrap().id;

            let mut stmt = tx.prepare("
                INSERT OR REPLACE INTO euph_msgs (
                    room, id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                    user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
                )
                VALUES (
                    ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                    ?, ?, ?, ?, ?, ?, ?, ?, ?
                )
            ")?;
            for msg in msgs {
                stmt.execute(params![
                    room,
                    msg.id,
                    msg.parent,
                    msg.previous_edit_id,
                    msg.time,
                    msg.content,
                    msg.encryption_key_id,
                    msg.edited,
                    msg.deleted,
                    msg.truncated,
                    msg.sender.id.0,
                    msg.sender.name,
                    msg.sender.server_id,
                    msg.sender.server_era,
                    msg.sender.session_id,
                    msg.sender.is_staff,
                    msg.sender.is_manager,
                    msg.sender.client_address,
                    msg.sender.real_client_address,
                ])?;
            }

            let last_msg_id = next_msg_id.unwrap_or(last_msg_id);
            Self::add_span(&tx, &room, Some(first_msg_id), Some(last_msg_id))?;
        }

        tx.commit()?;
        Ok(())
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
                WITH RECURSIVE
                path (room, id) AS (
                    VALUES (?, ?)
                UNION
                    SELECT room, parent
                    FROM euph_msgs
                    JOIN path USING (room, id)
                    WHERE parent IS NOT NULL
                )
                SELECT id
                FROM path
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, id], |row| row.get(0))?
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
                WITH RECURSIVE
                tree (room, id) AS (
                    VALUES (?, ?)
                UNION
                    SELECT euph_msgs.room, euph_msgs.id
                    FROM euph_msgs
                    JOIN tree
                        ON tree.room = euph_msgs.room
                        AND tree.id = euph_msgs.parent
                )
                SELECT id, parent, time, name, content
                FROM euph_msgs
                JOIN tree USING (room, id)
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, root], |row| {
                Ok(EuphMsg {
                    id: row.get(0)?,
                    parent: row.get(1)?,
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
            .query_row(params![room, root], |row| row.get(0))
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
            .query_row(params![room, root], |row| row.get(0))
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
            .query_row([room], |row| row.get(0))
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
            .query_row([room], |row| row.get(0))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }
}
