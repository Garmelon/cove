use std::mem;
use std::str::FromStr;

use async_trait::async_trait;
use cookie::{Cookie, CookieJar};
use euphoxide::api::{Message, SessionView, Snowflake, Time, UserId};
use rusqlite::types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{named_params, params, Connection, OptionalExtension, ToSql, Transaction};
use time::OffsetDateTime;
use tokio::sync::oneshot;

use crate::euph::SmallMessage;
use crate::store::{MsgStore, Path, Tree};

use super::{Request, Vault};

/// Wrapper for [`Snowflake`] that implements useful rusqlite traits.
struct WSnowflake(Snowflake);

impl ToSql for WSnowflake {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0 .0.to_sql()
    }
}

impl FromSql for WSnowflake {
    fn column_result(value: ValueRef<'_>) -> Result<Self, FromSqlError> {
        u64::column_result(value).map(|v| Self(Snowflake(v)))
    }
}

/// Wrapper for [`Time`] that implements useful rusqlite traits.
struct WTime(Time);

impl ToSql for WTime {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let timestamp = self.0 .0.unix_timestamp();
        Ok(ToSqlOutput::Owned(Value::Integer(timestamp)))
    }
}

impl FromSql for WTime {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let timestamp = i64::column_result(value)?;
        Ok(Self(Time(
            OffsetDateTime::from_unix_timestamp(timestamp).expect("timestamp in range"),
        )))
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

    pub fn add_message(
        &self,
        msg: Message,
        prev_msg: Option<Snowflake>,
        own_user_id: Option<UserId>,
    ) {
        let request = EuphRequest::AddMsg {
            room: self.room.clone(),
            msg: Box::new(msg),
            prev_msg,
            own_user_id,
        };
        let _ = self.vault.tx.send(request.into());
    }

    pub fn add_messages(
        &self,
        msgs: Vec<Message>,
        next_msg: Option<Snowflake>,
        own_user_id: Option<UserId>,
    ) {
        let request = EuphRequest::AddMsgs {
            room: self.room.clone(),
            msgs,
            next_msg,
            own_user_id,
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

    pub async fn chunk_at_offset(&self, amount: usize, offset: usize) -> Vec<Message> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetChunkAtOffset {
            room: self.room.clone(),
            amount,
            offset,
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

    async fn msg(&self, id: &Snowflake) -> Option<SmallMessage> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetMsg {
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

    async fn oldest_unseen_msg_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetOldestUnseenMsgId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn newest_unseen_msg_id(&self) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetNewestUnseenMsgId {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn older_unseen_msg_id(&self, id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetOlderUnseenMsgId {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn newer_unseen_msg_id(&self, id: &Snowflake) -> Option<Snowflake> {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetNewerUnseenMsgId {
            room: self.room.clone(),
            id: *id,
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn unseen_msgs_count(&self) -> usize {
        // TODO vault::Error
        let (tx, rx) = oneshot::channel();
        let request = EuphRequest::GetUnseenMsgsCount {
            room: self.room.clone(),
            result: tx,
        };
        let _ = self.vault.tx.send(request.into());
        rx.await.unwrap()
    }

    async fn set_seen(&self, id: &Snowflake, seen: bool) {
        let request = EuphRequest::SetSeen {
            room: self.room.clone(),
            id: *id,
            seen,
        };
        let _ = self.vault.tx.send(request.into());
    }

    async fn set_older_seen(&self, id: &Snowflake, seen: bool) {
        let request = EuphRequest::SetOlderSeen {
            room: self.room.clone(),
            id: *id,
            seen,
        };
        let _ = self.vault.tx.send(request.into());
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
        own_user_id: Option<UserId>,
    },
    AddMsgs {
        room: String,
        msgs: Vec<Message>,
        next_msg: Option<Snowflake>,
        own_user_id: Option<UserId>,
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
    GetMsg {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<SmallMessage>>,
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
    GetOlderUnseenMsgId {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetOldestUnseenMsgId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetNewestUnseenMsgId {
        room: String,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetNewerUnseenMsgId {
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<Snowflake>>,
    },
    GetUnseenMsgsCount {
        room: String,
        result: oneshot::Sender<usize>,
    },
    SetSeen {
        room: String,
        id: Snowflake,
        seen: bool,
    },
    SetOlderSeen {
        room: String,
        id: Snowflake,
        seen: bool,
    },
    GetChunkAtOffset {
        room: String,
        amount: usize,
        offset: usize,
        result: oneshot::Sender<Vec<Message>>,
    },
}

impl EuphRequest {
    pub(super) fn perform(self, conn: &mut Connection) {
        let result = match self {
            Self::GetCookies { result } => Self::get_cookies(conn, result),
            Self::SetCookies { cookies } => Self::set_cookies(conn, cookies),
            Self::GetRooms { result } => Self::get_rooms(conn, result),
            Self::Join { room, time } => Self::join(conn, room, time),
            Self::Delete { room } => Self::delete(conn, room),
            Self::AddMsg {
                room,
                msg,
                prev_msg,
                own_user_id,
            } => Self::add_msg(conn, room, *msg, prev_msg, own_user_id),
            Self::AddMsgs {
                room,
                msgs,
                next_msg,
                own_user_id,
            } => Self::add_msgs(conn, room, msgs, next_msg, own_user_id),
            Self::GetLastSpan { room, result } => Self::get_last_span(conn, room, result),
            Self::GetPath { room, id, result } => Self::get_path(conn, room, id, result),
            Self::GetMsg { room, id, result } => Self::get_msg(conn, room, id, result),
            Self::GetTree { room, root, result } => Self::get_tree(conn, room, root, result),
            Self::GetFirstTreeId { room, result } => Self::get_first_tree_id(conn, room, result),
            Self::GetLastTreeId { room, result } => Self::get_last_tree_id(conn, room, result),
            Self::GetPrevTreeId { room, root, result } => {
                Self::get_prev_tree_id(conn, room, root, result)
            }
            Self::GetNextTreeId { room, root, result } => {
                Self::get_next_tree_id(conn, room, root, result)
            }
            Self::GetOldestMsgId { room, result } => Self::get_oldest_msg_id(conn, room, result),
            Self::GetNewestMsgId { room, result } => Self::get_newest_msg_id(conn, room, result),
            Self::GetOlderMsgId { room, id, result } => {
                Self::get_older_msg_id(conn, room, id, result)
            }
            Self::GetNewerMsgId { room, id, result } => {
                Self::get_newer_msg_id(conn, room, id, result)
            }
            Self::GetOldestUnseenMsgId { room, result } => {
                Self::get_oldest_unseen_msg_id(conn, room, result)
            }
            Self::GetNewestUnseenMsgId { room, result } => {
                Self::get_newest_unseen_msg_id(conn, room, result)
            }
            Self::GetOlderUnseenMsgId { room, id, result } => {
                Self::get_older_unseen_msg_id(conn, room, id, result)
            }
            Self::GetNewerUnseenMsgId { room, id, result } => {
                Self::get_newer_unseen_msg_id(conn, room, id, result)
            }
            Self::GetUnseenMsgsCount { room, result } => {
                Self::get_unseen_msgs_count(conn, room, result)
            }
            Self::SetSeen { room, id, seen } => Self::set_seen(conn, room, id, seen),
            Self::SetOlderSeen { room, id, seen } => Self::set_older_seen(conn, room, id, seen),
            Self::GetChunkAtOffset {
                room,
                amount,
                offset,
                result,
            } => Self::get_chunk_at_offset(conn, room, amount, offset, result),
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
            named_params! {":room": room, ":time": WTime(time)},
        )?;
        Ok(())
    }

    fn delete(conn: &mut Connection, room: String) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        tx.execute(
            "
            DELETE FROM euph_rooms
            WHERE room = ?
            ",
            [&room],
        )?;

        tx.commit()?;
        Ok(())
    }

    fn insert_msgs(
        tx: &Transaction<'_>,
        room: &str,
        own_user_id: &Option<UserId>,
        msgs: Vec<Message>,
    ) -> rusqlite::Result<()> {
        let mut insert_msg = tx.prepare(
            "
            INSERT INTO euph_msgs (
                room, id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address,
                seen
            )
            VALUES (
                :room, :id, :parent, :previous_edit_id, :time, :content, :encryption_key_id, :edited, :deleted, :truncated,
                :user_id, :name, :server_id, :server_era, :session_id, :is_staff, :is_manager, :client_address, :real_client_address,
                (:user_id == :own_user_id OR EXISTS(
                    SELECT 1
                    FROM euph_rooms
                    WHERE room = :room
                    AND :time < first_joined
                ))
            )
            ON CONFLICT (room, id) DO UPDATE
            SET
                room = :room,
                id = :id,
                parent = :parent,
                previous_edit_id = :previous_edit_id,
                time = :time,
                content = :content,
                encryption_key_id = :encryption_key_id,
                edited = :edited,
                deleted = :deleted,
                truncated = :truncated,

                user_id = :user_id,
                name = :name,
                server_id = :server_id,
                server_era = :server_era,
                session_id = :session_id,
                is_staff = :is_staff,
                is_manager = :is_manager,
                client_address = :client_address,
                real_client_address = :real_client_address
            "
        )?;

        let own_user_id = own_user_id.as_ref().map(|u| &u.0);
        for msg in msgs {
            insert_msg.execute(named_params! {
                ":room": room,
                ":id": WSnowflake(msg.id),
                ":parent": msg.parent.map(WSnowflake),
                ":previous_edit_id": msg.previous_edit_id.map(WSnowflake),
                ":time": WTime(msg.time),
                ":content": msg.content,
                ":encryption_key_id": msg.encryption_key_id,
                ":edited": msg.edited.map(WTime),
                ":deleted": msg.deleted.map(WTime),
                ":truncated": msg.truncated,
                ":user_id": msg.sender.id.0,
                ":name": msg.sender.name,
                ":server_id": msg.sender.server_id,
                ":server_era": msg.sender.server_era,
                ":session_id": msg.sender.session_id,
                ":is_staff": msg.sender.is_staff,
                ":is_manager": msg.sender.is_manager,
                ":client_address": msg.sender.client_address,
                ":real_client_address": msg.sender.real_client_address,
                ":own_user_id": own_user_id, // May be NULL
            })?;
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
                let start = row.get::<_, Option<WSnowflake>>(0)?.map(|s| s.0);
                let end = row.get::<_, Option<WSnowflake>>(1)?.map(|s| s.0);
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
            stmt.execute(params![room, start.map(WSnowflake), end.map(WSnowflake)])?;
        }

        Ok(())
    }

    fn add_msg(
        conn: &mut Connection,
        room: String,
        msg: Message,
        prev_msg: Option<Snowflake>,
        own_user_id: Option<UserId>,
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        let end = msg.id;
        Self::insert_msgs(&tx, &room, &own_user_id, vec![msg])?;
        Self::add_span(&tx, &room, prev_msg, Some(end))?;

        tx.commit()?;
        Ok(())
    }

    fn add_msgs(
        conn: &mut Connection,
        room: String,
        msgs: Vec<Message>,
        next_msg_id: Option<Snowflake>,
        own_user_id: Option<UserId>,
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        if msgs.is_empty() {
            Self::add_span(&tx, &room, None, next_msg_id)?;
        } else {
            let first_msg_id = msgs.first().unwrap().id;
            let last_msg_id = msgs.last().unwrap().id;

            Self::insert_msgs(&tx, &room, &own_user_id, msgs)?;

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
            .query_row([room], |row| {
                Ok((
                    row.get::<_, Option<WSnowflake>>(0)?.map(|s| s.0),
                    row.get::<_, Option<WSnowflake>>(1)?.map(|s| s.0),
                ))
            })
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
            .query_map(params![room, WSnowflake(id)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })?
            .collect::<rusqlite::Result<_>>()?;
        let path = Path::new(path);
        let _ = result.send(path);
        Ok(())
    }

    fn get_msg(
        conn: &Connection,
        room: String,
        id: Snowflake,
        result: oneshot::Sender<Option<SmallMessage>>,
    ) -> rusqlite::Result<()> {
        let msg = conn
            .query_row(
                "
                SELECT id, parent, time, name, content, seen
                FROM euph_msgs
                WHERE room = ?
                AND id = ?
                ",
                params![room, WSnowflake(id)],
                |row| {
                    Ok(SmallMessage {
                        id: row.get::<_, WSnowflake>(0)?.0,
                        parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| s.0),
                        time: row.get::<_, WTime>(2)?.0,
                        nick: row.get(3)?,
                        content: row.get(4)?,
                        seen: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let _ = result.send(msg);
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
                SELECT id, parent, time, name, content, seen
                FROM euph_msgs
                JOIN tree USING (room, id)
                ORDER BY id ASC
                ",
            )?
            .query_map(params![room, WSnowflake(root)], |row| {
                Ok(SmallMessage {
                    id: row.get::<_, WSnowflake>(0)?.0,
                    parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| s.0),
                    time: row.get::<_, WTime>(2)?.0,
                    nick: row.get(3)?,
                    content: row.get(4)?,
                    seen: row.get(5)?,
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
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
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
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
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
            .query_row(params![room, WSnowflake(root)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
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
            .query_row(params![room, WSnowflake(root)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
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
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
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
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
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
            .query_row(params![room, WSnowflake(id)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
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
            .query_row(params![room, WSnowflake(id)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_oldest_unseen_msg_id(
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
                AND NOT seen
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_newest_unseen_msg_id(
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
                AND NOT seen
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row([room], |row| row.get::<_, WSnowflake>(0).map(|s| s.0))
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_older_unseen_msg_id(
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
                AND NOT seen
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(params![room, WSnowflake(id)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_newer_unseen_msg_id(
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
                AND NOT seen
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(params![room, WSnowflake(id)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| s.0)
            })
            .optional()?;
        let _ = result.send(tree);
        Ok(())
    }

    fn get_unseen_msgs_count(
        conn: &Connection,
        room: String,
        result: oneshot::Sender<usize>,
    ) -> rusqlite::Result<()> {
        let amount = conn
            .prepare(
                "
                SELECT amount
                FROM euph_unseen_counts
                WHERE room = ?
                ",
            )?
            .query_row(params![room], |row| row.get(0))
            .optional()?
            .unwrap_or(0);
        let _ = result.send(amount);
        Ok(())
    }

    fn set_seen(
        conn: &Connection,
        room: String,
        id: Snowflake,
        seen: bool,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE room = :room
            AND id = :id
            ",
            named_params! { ":room": room, ":id": WSnowflake(id), ":seen": seen },
        )?;
        Ok(())
    }

    fn set_older_seen(
        conn: &Connection,
        room: String,
        id: Snowflake,
        seen: bool,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE room = :room
            AND id <= :id
            AND seen != :seen
            ",
            named_params! { ":room": room, ":id": WSnowflake(id), ":seen": seen },
        )?;
        Ok(())
    }

    fn get_chunk_at_offset(
        conn: &Connection,
        room: String,
        amount: usize,
        offset: usize,
        result: oneshot::Sender<Vec<Message>>,
    ) -> rusqlite::Result<()> {
        let mut query = conn.prepare(
            "
            SELECT
                id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
            FROM euph_msgs
            WHERE room = ?
            ORDER BY id ASC
            LIMIT ?
            OFFSET ?
            ",
        )?;

        let messages = query
            .query_map(params![room, amount, offset], |row| {
                Ok(Message {
                    id: row.get::<_, WSnowflake>(0)?.0,
                    parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| s.0),
                    previous_edit_id: row.get::<_, Option<WSnowflake>>(2)?.map(|s| s.0),
                    time: row.get::<_, WTime>(3)?.0,
                    content: row.get(4)?,
                    encryption_key_id: row.get(5)?,
                    edited: row.get::<_, Option<WTime>>(6)?.map(|t| t.0),
                    deleted: row.get::<_, Option<WTime>>(7)?.map(|t| t.0),
                    truncated: row.get(8)?,
                    sender: SessionView {
                        id: UserId(row.get(9)?),
                        name: row.get(10)?,
                        server_id: row.get(11)?,
                        server_era: row.get(12)?,
                        session_id: row.get(13)?,
                        is_staff: row.get(14)?,
                        is_manager: row.get(15)?,
                        client_address: row.get(16)?,
                        real_client_address: row.get(17)?,
                    },
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let _ = result.send(messages);
        Ok(())
    }
}
