//! Asynchronous events.

use serde::{Deserialize, Serialize};

use super::{AuthOption, Message, PersonalAccountView, SessionView, Snowflake, Time, UserId};

/// Indicates that access to a room is denied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BounceEvent {
    /// The reason why access was denied.
    pub reason: Option<String>,
    /// Authentication options that may be used.
    pub auth_options: Option<Vec<AuthOption>>,
    /// Internal use only.
    pub agent_id: Option<UserId>,
    /// Internal use only.
    pub ip: Option<String>,
}

/// Indicates that the session is being closed. The client will subsequently be
/// disconnected.
///
/// If the disconnect reason is `authentication changed`, the client should
/// immediately reconnect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectEvent {
    /// The reason for disconnection.
    pub reason: String,
}

/// Sent by the server to the client when a session is started.
///
/// It includes information about the client's authentication and associated
/// identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloEvent {
    /// The id of the agent or account logged into this session.
    pub id: UserId,
    /// Details about the user's account, if the session is logged in.
    pub account: Option<PersonalAccountView>,
    /// Details about the session.
    pub session: SessionView,
    /// If true, then the account has an explicit access grant to the current
    /// room.
    pub account_has_access: Option<bool>,
    /// Whether the account's email address has been verified.
    pub account_email_verified: Option<bool>,
    /// If true, the session is connected to a private room.
    pub room_is_private: bool,
    /// The version of the code being run and served by the server.
    pub version: String,
}

/// Indicates a session just joined the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinEvent(pub SessionView);

/// Sent to all sessions of an agent when that agent is logged in (except for
/// the session that issued the login command).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginEvent {
    pub account_id: Snowflake,
}

/// Sent to all sessions of an agent when that agent is logged out (except for
/// the session that issued the logout command).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutEvent;

/// Indicates some server-side event that impacts the presence of sessions in a
/// room.
///
/// If the network event type is `partition`, then this should be treated as a
/// [`PartEvent`] for all sessions connected to the same server id/era combo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    /// The type of network event; for now, always `partition`.
    pub r#type: String,
    /// The id of the affected server.
    pub server_id: String,
    /// The era of the affected server.
    pub server_era: String,
}

/// Announces a nick change by another session in the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NickEvent {
    /// The id of the session this name applies to.
    pub session_id: String,
    /// The id of the agent or account logged into the session.
    pub id: UserId,
    /// The previous name associated with the session.
    pub from: String,
    /// The name associated with the session henceforth.
    pub to: String,
}

/// Indicates that a message in the room has been modified or deleted.
///
/// If the client offers a user interface and the indicated message is currently
/// displayed, it should update its display accordingly.
///
/// The event packet includes a snapshot of the message post-edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditMessageEvent {
    /// The id of the edit.
    pub edit_id: Snowflake,
    /// The snapshot of the message post-edit.
    #[serde(flatten)]
    pub message: Message,
}

/// Indicates a session just disconnected from the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartEvent(pub SessionView);

/// Represents a server-to-client ping.
///
/// The client should send back a ping-reply with the same value for the time
/// field as soon as possible (or risk disconnection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingEvent {
    /// A unix timestamp according to the server's clock.
    pub time: Time,
    /// The expected time of the next ping event, according to the server's
    /// clock.
    pub next: Time,
}

/// Informs the client that another user wants to chat with them privately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmInitiateEvent {
    /// The id of the user inviting the client to chat privately.
    pub from: UserId,
    /// The nick of the inviting user.
    pub from_nick: String,
    /// The room where the invitation was sent from.
    pub from_room: String,
    /// The private chat can be accessed at `/room/pm:<pm_id>`.
    pub pm_id: Snowflake,
}

/// Indicates a message received by the room from another session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEvent(pub Message);

/// Indicates that a session has successfully joined a room.
///
/// It also offers a snapshot of the room’s state and recent history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEvent {
    /// The id of the agent or account logged into this session.
    pub identity: UserId,
    /// The globally unique id of this session.
    pub session_id: String,
    /// The server’s version identifier.
    pub version: String,
    /// The list of all other sessions joined to the room (excluding this
    /// session).
    pub listing: Vec<SessionView>,
    /// The most recent messages posted to the room (currently up to 100).
    pub log: Vec<Message>,
    /// The acting nick of the session; if omitted, client set nick before
    /// speaking.
    pub nick: Option<String>,
    /// If given, this room is for private chat with the given nick.
    pub pm_with_nick: Option<String>,
    /// If given, this room is for private chat with the given user.
    pub pm_with_user_id: Option<String>,
}
