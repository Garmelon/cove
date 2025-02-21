use std::{fmt, mem, str::FromStr};

use async_trait::async_trait;
use cookie::{Cookie, CookieJar};
use euphoxide::api::{Message, MessageId, SessionId, SessionView, Snowflake, Time, UserId};
use rusqlite::{
    Connection, OptionalExtension, Row, ToSql, Transaction, named_params, params,
    types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef},
};
use vault::Action;

use crate::{
    euph::SmallMessage,
    store::{MsgStore, Path, Tree},
};

/// Wrapper for [`Snowflake`] that implements useful rusqlite traits.
struct WSnowflake(Snowflake);

impl ToSql for WSnowflake {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.0.to_sql()
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
        let timestamp = self.0.0;
        Ok(ToSqlOutput::Owned(Value::Integer(timestamp)))
    }
}

impl FromSql for WTime {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let timestamp = i64::column_result(value)?;
        Ok(Self(Time(timestamp)))
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct RoomIdentifier {
    pub domain: String,
    pub name: String,
}

impl fmt::Debug for RoomIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "&{}@{}", self.name, self.domain)
    }
}

impl RoomIdentifier {
    pub fn new(domain: String, name: String) -> Self {
        Self { domain, name }
    }
}

///////////////
// EuphVault //
///////////////

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

    pub fn room(&self, room: RoomIdentifier) -> EuphRoomVault {
        EuphRoomVault {
            vault: self.clone(),
            room,
        }
    }
}

macro_rules! euph_vault_actions {
    ( $(
        $struct:ident : $fn:ident ( $( $arg:ident : $arg_ty:ty ),* ) -> $res:ty ;
    )* ) => {
        $(
            struct $struct {
                $( $arg: $arg_ty, )*
            }
        )*

        impl EuphVault {
            $(
                pub async fn $fn(&self, $( $arg: $arg_ty, )* ) -> Result<$res, vault::tokio::Error<rusqlite::Error>> {
                    self.vault.tokio_vault.execute($struct { $( $arg, )* }).await
                }
            )*
        }
    };
}

euph_vault_actions! {
    GetCookies : cookies(domain: String) -> CookieJar;
    SetCookies : set_cookies(domain: String, cookies: CookieJar) -> ();
    ClearCookies : clear_cookies(domain: Option<String>) -> ();
    GetRooms : rooms() -> Vec<RoomIdentifier>;
    GetTotalUnseenMsgsCount : total_unseen_msgs_count() -> usize;
}

impl Action for GetCookies {
    type Output = CookieJar;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let cookies = conn
            .prepare(
                "
                SELECT cookie
                FROM euph_cookies
                WHERE domain = ?
                ",
            )?
            .query_map([self.domain], |row| {
                let cookie_str: String = row.get(0)?;
                Ok(Cookie::from_str(&cookie_str).expect("cookie in db is valid"))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut cookie_jar = CookieJar::new();
        for cookie in cookies {
            cookie_jar.add_original(cookie);
        }
        Ok(cookie_jar)
    }
}

impl Action for SetCookies {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let tx = conn.transaction()?;

        // Since euphoria sets all cookies on every response, we can just delete
        // all previous cookies.
        tx.execute(
            "
            DELETE FROM euph_cookies
            WHERE domain = ?",
            [&self.domain],
        )?;

        let mut insert_cookie = tx.prepare(
            "
            INSERT INTO euph_cookies (domain, cookie)
            VALUES (?, ?)
            ",
        )?;
        for cookie in self.cookies.iter() {
            insert_cookie.execute(params![self.domain, format!("{cookie}")])?;
        }
        drop(insert_cookie);

        tx.commit()?;
        Ok(())
    }
}

impl Action for ClearCookies {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        if let Some(domain) = self.domain {
            conn.execute("DELETE FROM euph_cookies WHERE domain = ?", [domain])?;
        } else {
            conn.execute_batch("DELETE FROM euph_cookies")?;
        }

        Ok(())
    }
}

impl Action for GetRooms {
    type Output = Vec<RoomIdentifier>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.prepare(
            "
                SELECT domain, room
                FROM euph_rooms
                ",
        )?
        .query_map([], |row| {
            Ok(RoomIdentifier {
                domain: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()
    }
}

impl Action for GetTotalUnseenMsgsCount {
    type Output = usize;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.prepare(
            "
                SELECT COALESCE(SUM(amount), 0)
                FROM euph_unseen_counts
                ",
        )?
        .query_row([], |row| row.get(0))
    }
}

///////////////////
// EuphRoomVault //
///////////////////

#[derive(Debug, Clone)]
pub struct EuphRoomVault {
    vault: EuphVault,
    room: RoomIdentifier,
}

impl EuphRoomVault {
    pub fn vault(&self) -> &EuphVault {
        &self.vault
    }

    pub fn room(&self) -> &RoomIdentifier {
        &self.room
    }
}

macro_rules! euph_room_vault_actions {
    ( $(
        $struct:ident : $fn:ident ( $( $arg:ident : $arg_ty:ty ),* ) -> $res:ty ;
    )* ) => {
        $(
            struct $struct {
                room: RoomIdentifier,
                $( $arg: $arg_ty, )*
            }
        )*

        impl EuphRoomVault {
            $(
                pub async fn $fn(&self, $( $arg: $arg_ty, )* ) -> Result<$res, vault::tokio::Error<rusqlite::Error>> {
                    self.vault.vault.tokio_vault.execute($struct {
                        room: self.room.clone(),
                        $( $arg, )*
                    }).await
                }
            )*
        }
    };
}

euph_room_vault_actions! {
    // Room
    Join : join(time: Time) -> ();
    Delete : delete() -> ();

    // Message
    AddMsg : add_msg(msg: Box<Message>, prev_msg_id: Option<MessageId>, own_user_id: Option<UserId>) -> ();
    AddMsgs : add_msgs(msgs: Vec<Message>, next_msg_id: Option<MessageId>, own_user_id: Option<UserId>) -> ();
    GetLastSpan : last_span() -> Option<(Option<MessageId>, Option<MessageId>)>;
    GetPath : path(id: MessageId) -> Path<MessageId>;
    GetMsg : msg(id: MessageId) -> Option<SmallMessage>;
    GetFullMsg : full_msg(id: MessageId) -> Option<Message>;
    GetTree : tree(root_id: MessageId) -> Tree<SmallMessage>;
    GetFirstRootId : first_root_id() -> Option<MessageId>;
    GetLastRootId : last_root_id() -> Option<MessageId>;
    GetPrevRootId : prev_root_id(root_id: MessageId) -> Option<MessageId>;
    GetNextRootId : next_root_id(root_id: MessageId) -> Option<MessageId>;
    GetOldestMsgId : oldest_msg_id() -> Option<MessageId>;
    GetNewestMsgId : newest_msg_id() -> Option<MessageId>;
    GetOlderMsgId : older_msg_id(id: MessageId) -> Option<MessageId>;
    GetNewerMsgId : newer_msg_id(id: MessageId) -> Option<MessageId>;
    GetOldestUnseenMsgId : oldest_unseen_msg_id() -> Option<MessageId>;
    GetNewestUnseenMsgId : newest_unseen_msg_id() -> Option<MessageId>;
    GetOlderUnseenMsgId : older_unseen_msg_id(id: MessageId) -> Option<MessageId>;
    GetNewerUnseenMsgId : newer_unseen_msg_id(id: MessageId) -> Option<MessageId>;
    GetUnseenMsgsCount : unseen_msgs_count() -> usize;
    SetSeen : set_seen(id: MessageId, seen: bool) -> ();
    SetOlderSeen : set_older_seen(id: MessageId, seen: bool) -> ();
    GetChunkAfter : chunk_after(id: Option<MessageId>, amount: usize) -> Vec<Message>;
}

impl Action for Join {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.execute(
            "
            INSERT INTO euph_rooms (domain, room, first_joined, last_joined)
            VALUES (:domain, :room, :time, :time)
            ON CONFLICT (domain, room) DO UPDATE
            SET last_joined = :time
            ",
            named_params! {
                ":domain": self.room.domain,
                ":room": self.room.name,
                ":time": WTime(self.time),
            },
        )?;
        Ok(())
    }
}

impl Action for Delete {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.execute(
            "
            DELETE FROM euph_rooms
            WHERE domain = ?
            AND room = ?
            ",
            [&self.room.domain, &self.room.name],
        )?;
        Ok(())
    }
}

fn insert_msgs(
    tx: &Transaction<'_>,
    room: &RoomIdentifier,
    own_user_id: &Option<UserId>,
    msgs: Vec<Message>,
) -> rusqlite::Result<()> {
    let mut insert_msg = tx.prepare(
        "
        INSERT INTO euph_msgs (
            domain, room,
            id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
            user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address,
            seen
        )
        VALUES (
            :domain, :room,
            :id, :parent, :previous_edit_id, :time, :content, :encryption_key_id, :edited, :deleted, :truncated,
            :user_id, :name, :server_id, :server_era, :session_id, :is_staff, :is_manager, :client_address, :real_client_address,
            (:user_id == :own_user_id OR EXISTS(
                SELECT 1
                FROM euph_rooms
                WHERE domain = :domain
                AND room = :room
                AND :time < first_joined
            ))
        )
        ON CONFLICT (domain, room, id) DO UPDATE
        SET
            domain = :domain,
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
            ":domain": room.domain,
            ":room": room.name,
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
    room: &RoomIdentifier,
    start: Option<MessageId>,
    end: Option<MessageId>,
) -> rusqlite::Result<()> {
    // Retrieve all spans for the room
    let mut spans = tx
        .prepare(
            "
            SELECT start, end
            FROM euph_spans
            WHERE domain = ?
            AND room = ?
            ",
        )?
        .query_map([&room.domain, &room.name], |row| {
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
        WHERE domain = ?
        AND room = ?
        ",
        [&room.domain, &room.name],
    )?;

    // Re-insert combined spans for the room
    let mut stmt = tx.prepare(
        "
        INSERT INTO euph_spans (domain, room, start, end)
        VALUES (?, ?, ?, ?)
        ",
    )?;
    for (start, end) in result {
        stmt.execute(params![
            room.domain,
            room.name,
            start.map(|id| WSnowflake(id.0)),
            end.map(|id| WSnowflake(id.0))
        ])?;
    }

    Ok(())
}

impl Action for AddMsg {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let tx = conn.transaction()?;

        let end = self.msg.id;
        insert_msgs(&tx, &self.room, &self.own_user_id, vec![*self.msg])?;
        add_span(&tx, &self.room, self.prev_msg_id, Some(end))?;

        tx.commit()?;
        Ok(())
    }
}

impl Action for AddMsgs {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
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

impl Action for GetLastSpan {
    type Output = Option<(Option<MessageId>, Option<MessageId>)>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let span = conn
            .prepare(
                "
                SELECT start, end
                FROM euph_spans
                WHERE domain = ?
                AND room = ?
                ORDER BY start DESC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                Ok((
                    row.get::<_, Option<WSnowflake>>(0)?.map(|s| MessageId(s.0)),
                    row.get::<_, Option<WSnowflake>>(1)?.map(|s| MessageId(s.0)),
                ))
            })
            .optional()?;
        Ok(span)
    }
}

impl Action for GetPath {
    type Output = Path<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let path = conn
            .prepare(
                "
                WITH RECURSIVE
                path (domain, room, id) AS (
                    VALUES (?, ?, ?)
                UNION
                    SELECT domain, room, parent
                    FROM euph_msgs
                    JOIN path USING (domain, room, id)
                )
                SELECT id
                FROM path
                WHERE id IS NOT NULL
                ORDER BY id ASC
                ",
            )?
            .query_map(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Path::new(path))
    }
}

impl Action for GetMsg {
    type Output = Option<SmallMessage>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg = conn
            .query_row(
                "
                SELECT id, parent, time, name, content, seen
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND id = ?
                ",
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
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
        Ok(msg)
    }
}

impl Action for GetFullMsg {
    type Output = Option<Message>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let mut query = conn.prepare(
            "
            SELECT
                id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
            FROM euph_msgs
            WHERE domain = ?
            AND room = ?
            AND id = ?
            "
        )?;

        let msg = query
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| {
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
                },
            )
            .optional()?;
        Ok(msg)
    }
}

impl Action for GetTree {
    type Output = Tree<SmallMessage>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msgs = conn
            .prepare(
                "
                WITH RECURSIVE
                tree (domain, room, id) AS (
                    VALUES (?, ?, ?)
                UNION
                    SELECT euph_msgs.domain, euph_msgs.room, euph_msgs.id
                    FROM euph_msgs
                    JOIN tree
                        ON tree.domain = euph_msgs.domain
                        AND tree.room = euph_msgs.room
                        AND tree.id = euph_msgs.parent
                )
                SELECT id, parent, time, name, content, seen
                FROM euph_msgs
                JOIN tree USING (domain, room, id)
                ORDER BY id ASC
                ",
            )?
            .query_map(
                params![self.room.domain, self.room.name, WSnowflake(self.root_id.0)],
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
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Tree::new(self.root_id, msgs))
    }
}

impl Action for GetFirstRootId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let root_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE domain = ?
                AND room = ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(root_id)
    }
}

impl Action for GetLastRootId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let root_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE domain = ?
                AND room = ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(root_id)
    }
}

impl Action for GetPrevRootId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let root_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE domain = ?
                AND room = ?
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.root_id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(root_id)
    }
}

impl Action for GetNextRootId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let root_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_trees
                WHERE domain = ?
                AND room = ?
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.root_id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(root_id)
    }
}

impl Action for GetOldestMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetNewestMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetOlderMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(msg_id)
    }
}
impl Action for GetNewerMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetOldestUnseenMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND NOT seen
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetNewestUnseenMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND NOT seen
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row([&self.room.domain, &self.room.name], |row| {
                row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0))
            })
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetOlderUnseenMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND NOT seen
                AND id < ?
                ORDER BY id DESC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetNewerUnseenMsgId {
    type Output = Option<MessageId>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let msg_id = conn
            .prepare(
                "
                SELECT id
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND NOT seen
                AND id > ?
                ORDER BY id ASC
                LIMIT 1
                ",
            )?
            .query_row(
                params![self.room.domain, self.room.name, WSnowflake(self.id.0)],
                |row| row.get::<_, WSnowflake>(0).map(|s| MessageId(s.0)),
            )
            .optional()?;
        Ok(msg_id)
    }
}

impl Action for GetUnseenMsgsCount {
    type Output = usize;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        let amount = conn
            .prepare(
                "
                SELECT amount
                FROM euph_unseen_counts
                WHERE domain = ?
                AND room = ?
                ",
            )?
            .query_row(params![self.room.domain, self.room.name], |row| row.get(0))
            .optional()?
            .unwrap_or(0);
        Ok(amount)
    }
}

impl Action for SetSeen {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE domain = :domain
            AND room = :room
            AND id = :id
            ",
            named_params! {
                ":domain": self.room.domain,
                ":room": self.room.name,
                ":id": WSnowflake(self.id.0),
                ":seen": self.seen,
            },
        )?;
        Ok(())
    }
}

impl Action for SetOlderSeen {
    type Output = ();
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        conn.execute(
            "
            UPDATE euph_msgs
            SET seen = :seen
            WHERE domain = :domain
            AND room = :room
            AND id <= :id
            AND seen != :seen
            ",
            named_params! {
                ":domain": self.room.domain,
                ":room": self.room.name,
                ":id": WSnowflake(self.id.0),
                ":seen": self.seen,
            },
        )?;
        Ok(())
    }
}

impl Action for GetChunkAfter {
    type Output = Vec<Message>;
    type Error = rusqlite::Error;

    fn run(self, conn: &mut Connection) -> Result<Self::Output, Self::Error> {
        fn row2msg(row: &Row<'_>) -> rusqlite::Result<Message> {
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
        }

        let messages = if let Some(id) = self.id {
            conn.prepare("
                SELECT
                    id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                    user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                AND id > ?
                ORDER BY id ASC
                LIMIT ?
            ")?
            .query_map(params![self.room.domain, self.room.name, WSnowflake(id.0), self.amount], row2msg)?
            .collect::<rusqlite::Result<_>>()?
        } else {
            conn.prepare("
                SELECT
                    id, parent, previous_edit_id, time, content, encryption_key_id, edited, deleted, truncated,
                    user_id, name, server_id, server_era, session_id, is_staff, is_manager, client_address, real_client_address
                FROM euph_msgs
                WHERE domain = ?
                AND room = ?
                ORDER BY id ASC
                LIMIT ?
            ")?
            .query_map(params![self.room.domain, self.room.name, self.amount], row2msg)?
            .collect::<rusqlite::Result<_>>()?
        };

        Ok(messages)
    }
}

#[async_trait]
impl MsgStore<SmallMessage> for EuphRoomVault {
    type Error = vault::tokio::Error<rusqlite::Error>;

    async fn path(&self, id: &MessageId) -> Result<Path<MessageId>, Self::Error> {
        self.path(*id).await
    }

    async fn msg(&self, id: &MessageId) -> Result<Option<SmallMessage>, Self::Error> {
        self.msg(*id).await
    }

    async fn tree(&self, root_id: &MessageId) -> Result<Tree<SmallMessage>, Self::Error> {
        self.tree(*root_id).await
    }

    async fn first_root_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.first_root_id().await
    }

    async fn last_root_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.last_root_id().await
    }

    async fn prev_root_id(&self, root_id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.prev_root_id(*root_id).await
    }

    async fn next_root_id(&self, root_id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.next_root_id(*root_id).await
    }

    async fn oldest_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.oldest_msg_id().await
    }

    async fn newest_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.newest_msg_id().await
    }

    async fn older_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.older_msg_id(*id).await
    }

    async fn newer_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.newer_msg_id(*id).await
    }

    async fn oldest_unseen_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.oldest_unseen_msg_id().await
    }

    async fn newest_unseen_msg_id(&self) -> Result<Option<MessageId>, Self::Error> {
        self.newest_unseen_msg_id().await
    }

    async fn older_unseen_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.older_unseen_msg_id(*id).await
    }

    async fn newer_unseen_msg_id(&self, id: &MessageId) -> Result<Option<MessageId>, Self::Error> {
        self.newer_unseen_msg_id(*id).await
    }

    async fn unseen_msgs_count(&self) -> Result<usize, Self::Error> {
        self.unseen_msgs_count().await
    }

    async fn set_seen(&self, id: &MessageId, seen: bool) -> Result<(), Self::Error> {
        self.set_seen(*id, seen).await
    }

    async fn set_older_seen(&self, id: &MessageId, seen: bool) -> Result<(), Self::Error> {
        self.set_older_seen(*id, seen).await
    }
}
