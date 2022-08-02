use std::mem;
use std::str::FromStr;

use async_trait::async_trait;
use cookie::{Cookie, CookieJar};
use rusqlite::types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{named_params, params, Connection, OptionalExtension, ToSql, Transaction};
use time::OffsetDateTime;
use tokio::sync::oneshot;

use crate::euph::api::{Message, Snowflake, Time};
use crate::euph::SmallMessage;
use crate::store::{MsgStore, Path, Tree};

use super::{Request, Vault};

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
        let timestamp = self.0.unix_timestamp();
        Ok(ToSqlOutput::Owned(Value::Integer(timestamp)))
    }
}

impl FromSql for Time {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let timestamp = i64::column_result(value)?;
        Ok(Self(
            OffsetDateTime::from_unix_timestamp(timestamp).expect("timestamp in range"),
        ))
    }
}

impl From<EuphRequest> for Request {
    fn from(r: EuphRequest) -> Self {
        Self::Euph(r)
    }
}

impl Vault {
    pub async fn euph_cookies(&self) -> CookieJar {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetCookies { result: tx };
        let _ = self.tx.send(request.into());
        rx.await.unwrap()
    }

    pub fn set_euph_cookies(&self, cookies: CookieJar) {
        let request = EuphRequest::SetCookies { cookies };
        let _ = self.tx.send(request.into());
    }

    pub async fn euph_rooms(&self) -> Vec<String> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetRooms { result: tx };
        let _ = self.tx.send(request.into());
        rx.await.unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct EuphVault {
    pub(super) vault: Vault,
    pub(super) room: String,
}

impl EuphVault {
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub fn room(&self) -> &str {
        &self.room
    }

    pub fn join(&self, time: Time) {
        let request = EuphRequest::Join {
            room: self.room.clone(),
            time,
        };
        let _ = self.vault.tx.send(request.into());
    }

    pub fn delete(self) {
        let request = EuphRequest::Delete { room: self.room };
        let _ = self.vault.tx.send(request.into());
    }

    pub fn add_message(&self, msg: Message, prev_msg: Option<Snowflake>) {
        let request = EuphRequest::AddMsg {
            room: self.room.clone(),
            msg: Box::new(msg),
            prev_msg,
        };
        let _ = self.vault.tx.send(request.into());
    }

    pub fn add_messages(&self, msgs: Vec<Message>, next_msg: Option<Snowflake>) {
        let request = EuphRequest::AddMsgs {
            room: self.room.clone(),
            msgs,
            next_msg,
        };
        let _ = self.vault.tx.send(request.into());
    }

    pub async fn last_span(&self) -> Option<(Option<Snowflake>, Option<Snowflake>)> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetLastSpan {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }
}

#[async_trait]
impl MsgStore<SmallMessage> for EuphVault {
    async fn path(&self, id: &Snowflake) -> Path<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetPath {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn tree(&self, tree_id: &Snowflake) -> Tree<SmallMessage> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetTree {
            room: self.room.clone(),
            root: *tree_id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn first_tree_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetFirstTreeId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn last_tree_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetLastTreeId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn prev_tree_id(&self, tree_id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetPrevTreeId {
            room: self.room.clone(),
            root: *tree_id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn next_tree_id(&self, tree_id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetNextTreeId {
            room: self.room.clone(),
            root: *tree_id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn oldest_msg_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetOldestMsgId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn newest_msg_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetNewestMsgId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn older_msg_id(&self, id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetOlderMsgId {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn newer_msg_id(&self, id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetNewerMsgId {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }
}

pub(super) enum EuphRequest {
    /////////////
    // Cookies //
    /////////////
    GetCookies {
        result: oneshot::Sender<CookieJar>,
    },
    SetCookies {
        cookies: CookieJar,
    },

    ///////////
    // Rooms //
    ///////////
    GetRooms {
        result: oneshot::Sender<Vec<String>>,
    },
    Join {
        room: String,
        time: Time,
    },
    Delete {
        room: String,
    },

    //////////////
    // Messages //
    //////////////
    AddMsg {
        room: String,
        msg: Box<Message>,
        prev_msg: Option<Snowflake>,
    },
    AddMsgs {
        room: String,
        msgs: Vec<Message>,
        next_msg: Option<Snowflake>,
    },
    GetLastSpan {
        room: String,
        result: oneshot::Sender<Option<(Option<Snowflake>, Option<Snowflake>)>>,
    },
    GetPath {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Path<Snowflake>>,
    },
    GetTree {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Tree<SmallMessage>>,
    },
    GetFirstTreeId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetLastTreeId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetPrevTreeId {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetNextTreeId {
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetOldestMsgId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetNewestMsgId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetOlderMsgId {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetNewerMsgId {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
}

impl EuphRequest {
    pub(super) fn perform(self, conn: &mut Connection) {
        let result = match self {
            EuphRequest::GetCookies { result } => Self::get_cookies(conn, result),
            EuphRequest::SetCookies { cookies } => Self::set_cookies(conn, cookies),
            EuphRequest::GetRooms { result } => Self::get_rooms(conn, result),
            EuphRequest::Join { room, time } => Self::join(conn, room, time),
            EuphRequest::Delete { room } => Self::delete(conn, room),
            EuphRequest::AddMsg {
                room,
                msg,
                prev_msg,
            } => Self::add_msg(conn, room, *msg, prev_msg),
            EuphRequest::AddMsgs {
                room,
                msgs,
                next_msg,
            } => Self::add_msgs(conn, room, msgs, next_msg),
            EuphRequest::GetLastSpan { room, result } => Self::get_last_span(conn, room, result),
            EuphRequest::GetPath { room, id, result } => Self::get_path(conn, room, id, result),
            EuphRequest::GetTree { room, root, result } => Self::get_tree(conn, room, root, result),
            EuphRequest::GetFirstTreeId { room, result } => {
                Self::get_first_tree_id(conn, room, result)
            }
            EuphRequest::GetLastTreeId { room, result } => {
                Self::get_last_tree_id(conn, room, result)
            }
            EuphRequest::GetPrevTreeId { room, root, result } => {
                Self::get_prev_tree_id(conn, room, root, result)
            }
            EuphRequest::GetNextTreeId { room, root, result } => {
                Self::get_next_tree_id(conn, room, root, result)
            }
            EuphRequest::GetOldestMsgId { room, result } => {
                Self::get_oldest_msg_id(conn, room, result)
            }
            EuphRequest::GetNewestMsgId { room, result } => {
                Self::get_newest_msg_id(conn, room, result)
            }
            EuphRequest::GetOlderMsgId { room, id, result } => {
                Self::get_older_msg_id(conn, room, id, result)
            }
            EuphRequest::GetNewerMsgId { room, id, result } => {
                Self::get_newer_msg_id(conn, room, id, result)
            }
        };
        if let Err(e) = result {
            // If an error occurs here, the rest of the UI will likely panic and
            // crash soon. By printing this to stderr instead of logging it, we
            // can filter it out and read it later.
            // TODO Better vault error handling
            eprintln!("{e}");
        }
    }

    fn get_cookies(
        conn: &mut Connection,
        result: oneshot::Sender<CookieJar>,
    ) -> rusqlite::Result<()> {
        let cookies = conn
            .prepare(
                "
                SELECT cookie
                FROM euph_cookies
                ",
            )?
            .query_map([], |row| {
                let cookie_str: String = row.get(0)?;
                Ok(Cookie::from_str(&cookie_str).expect("cookie in db is valid"))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut cookie_jar = CookieJar::new();
        for cookie in cookies {
            cookie_jar.add_original(cookie);
        }

        let _ = result.send(cookie_jar);
        Ok(())
    }

    fn set_cookies(conn: &mut Connection, cookies: CookieJar) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        // Since euphoria sets all cookies on every response, we can just delete
        // all previous cookies.
        tx.execute_batch("DELETE FROM euph_cookies")?;

        let mut insert_cookie = tx.prepare(
            "
            INSERT INTO euph_cookies (cookie)
            VALUES (?)
            ",
        )?;
        for cookie in cookies.iter() {
            insert_cookie.execute([format!("{cookie}")])?;
        }
        drop(insert_cookie);

        tx.commit()?;
        Ok(())
    }

    fn get_rooms(
        conn: &mut Connection,
        result: oneshot::Sender<Vec<String>>,
    ) -> rusqlite::Result<()> {
        let rooms = conn
            .prepare(
                "
                SELECT room
                FROM euph_rooms
                ",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let _ = result.send(rooms);
        Ok(())
    }

    fn join(conn: &mut Connection, room: String, time: Time) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO euph_rooms (room, first_joined, last_joined)
            VALUES (:room, :time, :time)
            ON CONFLICT (room) DO UPDATE
            SET last_joined = :time
            ",
            named_params! {":room": room, ":time": time},
        )?;
        Ok(())
    }

    fn delete(conn: &mut Connection, room: String) -> rusqlite::Result<()> {
        conn.execute(
            "
            DELETE FROM euph_rooms
            WHERE room = ?
            ",
            [room],
        )?;
        Ok(())
    }

    fn insert_msgs(tx: &Transaction<'_>, room: &str, msgs: Vec<Message>) -> rusqlite::Result<()> {
        let mut insert_msg = tx.prepare(
            "
            INSERT OR REPLACE INTO euph_msgs (
                room, id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
            )
            VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            "
        )?;
        let mut delete_trees = tx.prepare(
            "
            DELETE FROM euph_trees
            WHERE room = ? AND id = ?
            ",
        )?;
        let mut insert_trees = tx.prepare(
            "
            INSERT OR IGNORE INTO euph_trees (room, id)
            SELECT *
            FROM (VALUES (:room, :id))
            WHERE NOT EXISTS(
                SELECT *
                FROM euph_msgs
                WHERE room = :room
                AND id = :id
                AND parent IS NOT NULL
            )
            ",
        )?;

        for msg in msgs {
            insert_msg.execute(params![
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

            if let Some(parent) = msg.parent {
                delete_trees.execute(params![room, msg.id])?;
                insert_trees.execute(named_params! {":room": room,":id": parent})?;
            } else {
                insert_trees.execute(named_params! {":room": room,":id": msg.id})?;
            }
        }

        Ok(())
    }

    fn add_span(
        tx: &Transaction<'_>,
        room: &str,
        start: Option<Snowflake>,
        end: Option<Snowflake>,
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
        spans.push((start, end));
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
        conn: &mut Connection,
        room: String,
        msg: Message,
        prev_msg: Option<Snowflake>,
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        let end = msg.id;
        Self::insert_msgs(&tx, &room, vec![msg])?;
        Self::add_span(&tx, &room, prev_msg, Some(end))?;

        tx.commit()?;
        Ok(())
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
            let last_msg_id = msgs.last().unwrap().id;

            Self::insert_msgs(&tx, &room, msgs)?;

            let end = next_msg_id.unwrap_or(last_msg_id);
            Self::add_span(&tx, &room, Some(first_msg_id), Some(end))?;
        }

        tx.commit()?;
        Ok(())
    }

    fn get_last_span(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<Option<(Option<Snowflake>, Option<Snowflake>)>>,
    ) -> rusqlite::Result<()> {
        let span = conn
            .prepare(
                "
                SELECT start, end
                FROM euph_spans
                WHERE room = ?
                ORDER BY start DESC
                LIMIT 1
                ",
            )?
            .query_row([room], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?;
        let _ = result.send(span);
        Ok(())
    }

    fn get_path(
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
                )
                SELECT id
                FROM path
                WHERE id IS NOT NULL
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let path = Path::new(path);
        let _ = result.send(path);
        Ok(())
    }

    fn get_tree(
        conn: &Connection,
        room: String,
        root: Snowflake,
        result: oneshot::Sender<Tree<SmallMessage>>,
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
                Ok(SmallMessage {
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

    fn get_first_tree_id(
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

    fn get_last_tree_id(
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

    fn get_prev_tree_id(
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

    fn get_next_tree_id(
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

    fn get_oldest_msg_id(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
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

    fn get_newest_msg_id(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
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

    fn get_older_msg_id(
        conn: &Connection,
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE room = ?
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(params![room, id], |row| row.get(0))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_newer_msg_id(
        conn: &Connection,
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    ) -> rusqlite::Result<()> {
        let tree = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE room = ?
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(params![room, id], |row| row.get(0))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }
}
