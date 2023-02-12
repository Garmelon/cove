use std::convert::Infallible;
use std::mem;
use std::str::FromStr;

use async_trait::async_trait;
use cookie::{Cookie, CookieJar};
use euphoxide::api::{Message, MessageId, SessionId, SessionView, Snowflake, Time, UserId};
use rusqlite::types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{named_params, params, Connection, OptionalExtension, ToSql, Transaction};
use time::OffsetDateTime;
use tokio::sync::oneshot;

use crate::euph::SmallMessage;
use crate::store::{MsgStore, Path, Tree};

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

#[derive(Debug, Clone)]
pub struct EuphVault {
    vault: super::Vault,
}

impl EuphVault {
    pub(crate) fn new(vault: super::Vault) -> Self {
        Self { vault }
    }

    pub fn vault(&self) -> &super::Vault {
        &self.vault
    }

    pub fn room(&self, name: String) -> EuphRoomVault {
        EuphRoomVault {
            vault: self.clone(),
            room: name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EuphRoomVault {
    vault: EuphVault,
    room: String,
}

impl EuphRoomVault {
    pub fn vault(&self) -> &EuphVault {
        &self.vault
    }

    pub fn room(&self) -> &str {
        &self.room
    }
}

#[async_trait]
impl MsgStore<SmallMessage> for EuphRoomVault {
    type Error = Infallible;

    async fn path(&self, id: &MessageId) -> Result<Path<MessageId>, Self::Error> {
        Ok(self.path(*id).await)
    }

    async fn msg(&self, id: &MessageId) -> Result<Option<SmallMessage>, Self::Error> {
        Ok(self.msg(*id).await)
    }

    async fn tree(&self, root_id: &MessageId) -> Result<Tree<SmallMessage>, Self::Error> {
        Ok(self.tree(*root_id).await)
    }

    async fn first_root_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.first_root_id().await)
    }

    async fn last_root_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.last_root_id().await)
    }

    async fn prev_root_id(&self, root_id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.prev_root_id(*root_id).await)
    }

    async fn next_root_id(&self, root_id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.next_root_id(*root_id).await)
    }

    async fn oldest_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.oldest_msg_id().await)
    }

    async fn newest_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.newest_msg_id().await)
    }

    async fn older_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.older_msg_id(*id).await)
    }

    async fn newer_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.newer_msg_id(*id).await)
    }

    async fn oldest_unseen_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.oldest_unseen_msg_id().await)
    }

    async fn newest_unseen_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.newest_unseen_msg_id().await)
    }

    async fn older_unseen_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.older_unseen_msg_id(*id).await)
    }

    async fn newer_unseen_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        Ok(self.newer_unseen_msg_id(*id).await)
    }

    async fn unseen_msgs_count(&self) -> Result<usize, Self::Error> {
        Ok(self.unseen_msgs_count().await)
    }

    async fn set_seen(&self, id: &MessageId, seen: bool) -> Result<(), Self::Error> {
        self.set_seen(*id, seen);
        Ok(())
    }

    async fn set_older_seen(&self, id: &MessageId, seen: bool) -> Result<(), Self::Error> {
        self.set_older_seen(*id, seen);
        Ok(())
    }
}

trait Request {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()>;
}

macro_rules! requests_vault_fn {
    ( $var:ident : $fn:ident( $( $arg:ident : $ty:ty ),* ) ) => {
        pub fn $fn(&self $( , $arg: $ty )* ) {
            let request = EuphRequest::$var($var { $( $arg, )* });
            let _ = self.vault.tx.send(super::Request::Euph(request));
        }
    };
    ( $var:ident : $fn:ident( $( $arg:ident : $ty:ty ),* ) -> $res:ty ) => {
        pub async fn $fn(&self $( , $arg: $ty )* ) -> $res {
            let (tx, rx) = oneshot::channel();
            let request = EuphRequest::$var($var {
                $( $arg, )*
                result: tx,
            });
            let _ = self.vault.tx.send(super::Request::Euph(request));
            rx.await.unwrap()
        }
    };
}

// This doesn't match the type of the `room` argument because that's apparently
// impossible to match to `String`. See also the readme of
// https://github.com/danielhenrymantilla/rust-defile for a description of this
// phenomenon and some examples.
macro_rules! requests_room_vault_fn {
    ( $fn:ident ( room: $mustbestring:ty $( , $arg:ident : $ty:ty )* ) ) => {
        pub fn $fn(&self $( , $arg: $ty )* ) {
            self.vault.$fn(self.room.clone() $( , $arg )* );
        }
    };
    ( $fn:ident ( room: $mustbestring:ty $( , $arg:ident : $ty:ty )* ) -> $res:ty ) => {
        pub async fn $fn(&self $( , $arg: $ty )* ) -> $res {
            self.vault.$fn(self.room.clone() $( , $arg )* ).await
        }
    };
    ( $( $tt:tt )* ) => { };
}

macro_rules! requests {
    ( $(
        $var:ident : $fn:ident ( $( $arg:ident : $ty:ty ),* ) $( -> $res:ty )? ;
    )* ) => {
        $(
            pub(super) struct $var {
                $( $arg: $ty, )*
                $( result: oneshot::Sender<$res>, )?
            }
        )*

        pub(super) enum EuphRequest {
            $( $var($var), )*
        }

        impl EuphRequest {
            pub(super) fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
                match self {
                    $( Self::$var(request) => request.perform(conn), )*
                }
            }
        }

        #[allow(dead_code)]
        impl EuphVault {
            $( requests_vault_fn!($var : $fn( $( $arg: $ty ),* ) $( -> $res )? ); )*
        }

        #[allow(dead_code)]
        impl EuphRoomVault {
            $( requests_room_vault_fn!($fn( $( $arg: $ty ),* ) $( -> $res )? ); )*
        }
    };
}

requests! {
    // Cookies
    GetCookies : cookies() -> CookieJar;
    SetCookies : set_cookies(cookies: CookieJar);

    // Rooms
    GetRooms : rooms() -> Vec<String>;
    Join : join(room: String, time: Time);
    Delete : delete(room: String);

    // Message
    AddMsg : add_msg(room: String, msg: Box<Message>, prev_msg_id: Option<MessageId>, own_user_id: Option<UserId>);
    AddMsgs : add_msgs(room: String, msgs: Vec<Message>, next_msg_id: Option<MessageId>, own_user_id: Option<UserId>);
    GetLastSpan : last_span(room: String) -> Option<(Option<MessageId>, Option<MessageId>)>;
    GetPath : path(room: String, id: MessageId) -> Path<MessageId>;
    GetMsg : msg(room: String, id: MessageId) -> Option<SmallMessage>;
    GetFullMsg : full_msg(room: String, id: MessageId) -> Option<Message>;
    GetTree : tree(room: String, root_id: MessageId) -> Tree<SmallMessage>;
    GetFirstRootId : first_root_id(room: String) -> Option<MessageId>;
    GetLastRootId : last_root_id(room: String) -> Option<MessageId>;
    GetPrevRootId : prev_root_id(room: String, root_id: MessageId) -> Option<MessageId>;
    GetNextRootId : next_root_id(room: String, root_id: MessageId) -> Option<MessageId>;
    GetOldestMsgId : oldest_msg_id(room: String) -> Option<MessageId>;
    GetNewestMsgId : newest_msg_id(room: String) -> Option<MessageId>;
    GetOlderMsgId : older_msg_id(room: String, id: MessageId) -> Option<MessageId>;
    GetNewerMsgId : newer_msg_id(room: String, id: MessageId) -> Option<MessageId>;
    GetOldestUnseenMsgId : oldest_unseen_msg_id(room: String) -> Option<MessageId>;
    GetNewestUnseenMsgId : newest_unseen_msg_id(room: String) -> Option<MessageId>;
    GetOlderUnseenMsgId : older_unseen_msg_id(room: String, id: MessageId) -> Option<MessageId>;
    GetNewerUnseenMsgId : newer_unseen_msg_id(room: String, id: MessageId) -> Option<MessageId>;
    GetUnseenMsgsCount : unseen_msgs_count(room: String) -> usize;
    SetSeen : set_seen(room: String, id: MessageId, seen: bool);
    SetOlderSeen : set_older_seen(room: String, id: MessageId, seen: bool);
    GetChunkAtOffset : chunk_at_offset(room: String, amount: usize, offset: usize) -> Vec<Message>;
}

impl Request for GetCookies {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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

        let _ = self.result.send(cookie_jar);
        Ok(())
    }
}

impl Request for SetCookies {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
        for cookie in self.cookies.iter() {
            insert_cookie.execute([format!("{cookie}")])?;
        }
        drop(insert_cookie);

        tx.commit()?;
        Ok(())
    }
}

impl Request for GetRooms {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let rooms = conn
            .prepare(
                "
                SELECT room
                FROM euph_rooms
                ",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let _ = self.result.send(rooms);
        Ok(())
    }
}

impl Request for Join {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO euph_rooms (room, first_joined, last_joined)
            VALUES (:room, :time, :time)
            ON CONFLICT (room) DO UPDATE
            SET last_joined = :time
            ",
            named_params! {":room": self.room, ":time": WTime(self.time)},
        )?;
        Ok(())
    }
}

impl Request for Delete {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            DELETE FROM euph_rooms
            WHERE room = ?
            ",
            [&self.room],
        )?;
        Ok(())
    }
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
            ":id": WSnowflake(msg.id.0),
            ":parent": msg.parent.map(|id| WSnowflake(id.0)),
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
            ":session_id": msg.sender.session_id.0,
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
    start: Option<MessageId>,
    end: Option<MessageId>,
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
            let start = row.get::<_, Option<WSnowflake>>(0)?.map(|s| MessageId(s.0));
            let end = row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0));
            Ok((start, end))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Add new span and sort spans lexicographically
    spans.push((start, end));
    spans.sort_unstable();

    // Combine overlapping spans (including newly added span)
    let mut cur_span: Option<(Option<MessageId>, Option<MessageId>)> = None;
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
        stmt.execute(params![
            room,
            start.map(|id| WSnowflake(id.0)),
            end.map(|id| WSnowflake(id.0))
        ])?;
    }

    Ok(())
}

impl Request for AddMsg {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        let end = self.msg.id;
        insert_msgs(&tx, &self.room, &self.own_user_id, vec![*self.msg])?;
        add_span(&tx, &self.room, self.prev_msg_id, Some(end))?;

        tx.commit()?;
        Ok(())
    }
}

impl Request for AddMsgs {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        if self.msgs.is_empty() {
            add_span(&tx, &self.room, None, self.next_msg_id)?;
        } else {
            let first_msg_id = self.msgs.first().unwrap().id;
            let last_msg_id = self.msgs.last().unwrap().id;

            insert_msgs(&tx, &self.room, &self.own_user_id, self.msgs)?;

            let end = self.next_msg_id.unwrap_or(last_msg_id);
            add_span(&tx, &self.room, Some(first_msg_id), Some(end))?;
        }

        tx.commit()?;
        Ok(())
    }
}

impl Request for GetLastSpan {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                Ok((
                    row.get::<_, Option<WSnowflake>>(0)?.map(|s| MessageId(s.0)),
                    row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
                ))
            })
            .optional()?;
        let _ = self.result.send(span);
        Ok(())
    }
}

impl Request for GetPath {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_map(params![self.room, WSnowflake(self.id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let path = Path::new(path);
        let _ = self.result.send(path);
        Ok(())
    }
}

impl Request for GetMsg {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let msg = conn
            .query_row(
                "
                SELECT id, parent, time, name, content, seen
                FROM euph_msgs
                WHERE room = ?
                AND id = ?
                ",
                params![self.room, WSnowflake(self.id.0)],
                |row| {
                    Ok(SmallMessage {
                        id: MessageId(row.get::<_, WSnowflake>(0)?.0),
                        parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
                        time: row.get::<_, WTime>(2)?.0,
                        nick: row.get(3)?,
                        content: row.get(4)?,
                        seen: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let _ = self.result.send(msg);
        Ok(())
    }
}

impl Request for GetFullMsg {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let mut query = conn.prepare(
            "
            SELECT
                id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
            FROM euph_msgs
            WHERE room = ?
            AND id = ?
            "
        )?;

        let msg = query
            .query_row(params![self.room, WSnowflake(self.id.0)], |row| {
                Ok(Message {
                    id: MessageId(row.get::<_, WSnowflake>(0)?.0),
                    parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
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
                        session_id: SessionId(row.get(13)?),
                        is_staff: row.get(14)?,
                        is_manager: row.get(15)?,
                        client_address: row.get(16)?,
                        real_client_address: row.get(17)?,
                    },
                })
            })
            .optional()?;
        let _ = self.result.send(msg);
        Ok(())
    }
}

impl Request for GetTree {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_map(params![self.room, WSnowflake(self.root_id.0)], |row| {
                Ok(SmallMessage {
                    id: MessageId(row.get::<_, WSnowflake>(0)?.0),
                    parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
                    time: row.get::<_, WTime>(2)?.0,
                    nick: row.get(3)?,
                    content: row.get(4)?,
                    seen: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let tree = Tree::new(self.root_id, msgs);
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetFirstRootId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetLastRootId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetPrevRootId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.root_id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetNextRootId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.root_id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetOldestMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetNewestMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetOlderMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}
impl Request for GetNewerMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetOldestUnseenMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetNewestUnseenMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row([self.room], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetOlderUnseenMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetNewerUnseenMsgId {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_row(params![self.room, WSnowflake(self.id.0)], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        let _ = self.result.send(tree);
        Ok(())
    }
}

impl Request for GetUnseenMsgsCount {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        let amount = conn
            .prepare(
                "
                SELECT amount
                FROM euph_unseen_counts
                WHERE room = ?
                ",
            )?
            .query_row(params![self.room], |row| row.get(0))
            .optional()?
            .unwrap_or(0);
        let _ = self.result.send(amount);
        Ok(())
    }
}

impl Request for SetSeen {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE room = :room
            AND id = :id
            ",
            named_params! { ":room": self.room, ":id": WSnowflake(self.id.0), ":seen": self.seen },
        )?;
        Ok(())
    }
}

impl Request for SetOlderSeen {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE room = :room
            AND id <= :id
            AND seen != :seen
            ",
            named_params! { ":room": self.room, ":id": WSnowflake(self.id.0), ":seen": self.seen },
        )?;
        Ok(())
    }
}

impl Request for GetChunkAtOffset {
    fn perform(self, conn: &mut Connection) -> rusqlite::Result<()> {
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
            .query_map(params![self.room, self.amount, self.offset], |row| {
                Ok(Message {
                    id: MessageId(row.get::<_, WSnowflake>(0)?.0),
                    parent: row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
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
                        session_id: SessionId(row.get(13)?),
                        is_staff: row.get(14)?,
                        is_manager: row.get(15)?,
                        client_address: row.get(16)?,
                        real_client_address: row.get(17)?,
                    },
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let _ = self.result.send(messages);
        Ok(())
    }
}
