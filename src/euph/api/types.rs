//! Field types.

// TODO Add newtype wrappers for different kinds of IDs?

// Serde's derive macros generate this warning and I can't turn it off locally,
// so I'm turning it off for the entire module.
#![allow(clippy::use_self)]

use std::fmt;

use serde::{de, ser, Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// Describes an account and its preferred name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountView {
    /// The id of the account.
    pub id: Snowflake,
    /// The name that the holder of the account goes by.
    pub name: String,
}

/// Mode of authentication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthOption {
    /// Authentication with a passcode, where a key is derived from the passcode
    /// to unlock an access grant.
    Passcode,
}

/// A node in a room's log.
///
/// It corresponds to a chat message, or a post, or any broadcasted event in a
/// room that should appear in the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The id of the message (unique within a room).
    pub id: Snowflake,
    /// The id of the message's parent, or null if top-level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Snowflake>,
    /// The edit id of the most recent edit of this message, or null if it's
    /// never been edited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_edit_id: Option<Snowflake>,
    /// The unix timestamp of when the message was posted.
    pub time: Time,
    /// The view of the sender's session.
    pub sender: SessionView,
    /// The content of the message (client-defined).
    pub content: String,
    /// The id of the key that encrypts the message in storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_key_id: Option<String>,
    /// The unix timestamp of when the message was last edited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited: Option<Time>,
    /// The unix timestamp of when the message was deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<Time>,
    /// If true, then the full content of this message is not included (see
    /// [`GetMessage`](super::GetMessage) to obtain the message with full
    /// content).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// The type of a packet.
///
/// Not all of these types have their corresponding data modeled as a struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PacketType {
    // Asynchronous events
    /// See [`BounceEvent`](super::BounceEvent).
    BounceEvent,
    /// See [`DisconnectEvent`](super::DisconnectEvent).
    DisconnectEvent,
    /// See [`HelloEvent`](super::HelloEvent).
    HelloEvent,
    /// See [`JoinEvent`](super::JoinEvent).
    JoinEvent,
    /// See [`LoginEvent`](super::LoginEvent).
    LoginEvent,
    /// See [`LogoutEvent`](super::LogoutEvent).
    LogoutEvent,
    /// See [`NetworkEvent`](super::NetworkEvent).
    NetworkEvent,
    /// See [`NickEvent`](super::NickEvent).
    NickEvent,
    /// See [`EditMessageEvent`](super::EditMessageEvent).
    EditMessageEvent,
    /// See [`PartEvent`](super::PartEvent).
    PartEvent,
    /// See [`PingEvent`](super::PingEvent).
    PingEvent,
    /// See [`PmInitiateEvent`](super::PmInitiateEvent).
    PmInitiateEvent,
    /// See [`SendEvent`](super::SendEvent).
    SendEvent,
    /// See [`SnapshotEvent`](super::SnapshotEvent).
    SnapshotEvent,

    // Session commands
    /// See [`Auth`](super::Auth).
    Auth,
    /// See [`AuthReply`](super::AuthReply).
    AuthReply,
    /// See [`Ping`](super::Ping).
    Ping,
    /// See [`PingReply`](super::PingReply).
    PingReply,

    // Chat room commands
    /// See [`GetMessage`](super::GetMessage).
    GetMessage,
    /// See [`GetMessageReply`](super::GetMessageReply).
    GetMessageReply,
    /// See [`Log`](super::Log).
    Log,
    /// See [`LogReply`](super::LogReply).
    LogReply,
    /// See [`Nick`](super::Nick).
    Nick,
    /// See [`NickReply`](super::NickReply).
    NickReply,
    /// See [`PmInitiate`](super::PmInitiate).
    PmInitiate,
    /// See [`PmInitiateReply`](super::PmInitiateReply).
    PmInitiateReply,
    /// See [`Send`](super::Send).
    Send,
    /// See [`SendReply`](super::SendReply).
    SendReply,
    /// See [`Who`](super::Who).
    Who,
    /// See [`WhoReply`](super::WhoReply).
    WhoReply,

    // Account commands
    /// Not implemented.
    ChangeEmail,
    /// Not implemented.
    ChangeEmailReply,
    /// Not implemented.
    ChangeName,
    /// Not implemented.
    ChangeNameReply,
    /// Not implemented.
    ChangePassword,
    /// Not implemented.
    ChangePasswordReply,
    /// Not implemented.
    Login,
    /// Not implemented.
    LoginReply,
    /// Not implemented.
    Logout,
    /// Not implemented.
    LogoutReply,
    /// Not implemented.
    RegisterAccount,
    /// Not implemented.
    RegisterAccountReply,
    /// Not implemented.
    ResendVerificationEmail,
    /// Not implemented.
    ResendVerificationEmailReply,
    /// Not implemented.
    ResetPassword,
    /// Not implemented.
    ResetPasswordReply,

    // Room host commands
    /// Not implemented.
    Ban,
    /// Not implemented.
    BanReply,
    /// Not implemented.
    EditMessage,
    /// Not implemented.
    EditMessageReply,
    /// Not implemented.
    GrantAccess,
    /// Not implemented.
    GrantAccessReply,
    /// Not implemented.
    GrantManager,
    /// Not implemented.
    GrantManagerReply,
    /// Not implemented.
    RevokeAccess,
    /// Not implemented.
    RevokeAccessReply,
    /// Not implemented.
    RevokeManager,
    /// Not implemented.
    RevokeManagerReply,
    /// Not implemented.
    Unban,
    /// Not implemented.
    UnbanReply,

    // Staff commands
    /// Not implemented.
    StaffCreateRoom,
    /// Not implemented.
    StaffCreateRoomReply,
    /// Not implemented.
    StaffEnrollOtp,
    /// Not implemented.
    StaffEnrollOtpReply,
    /// Not implemented.
    StaffGrantManager,
    /// Not implemented.
    StaffGrantManagerReply,
    /// Not implemented.
    StaffInvade,
    /// Not implemented.
    StaffInvadeReply,
    /// Not implemented.
    StaffLockRoom,
    /// Not implemented.
    StaffLockRoomReply,
    /// Not implemented.
    StaffRevokeAccess,
    /// Not implemented.
    StaffRevokeAccessReply,
    /// Not implemented.
    StaffValidateOtp,
    /// Not implemented.
    StaffValidateOtpReply,
    /// Not implemented.
    UnlockStaffCapability,
    /// Not implemented.
    UnlockStaffCapabilityReply,
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_value(self) {
            Ok(Value::String(s)) => write!(f, "{}", s),
            _ => Err(fmt::Error),
        }
    }
}

/// Describes an account to its owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalAccountView {
    /// The id of the account.
    pub id: Snowflake,
    /// The name that the holder of the account goes by.
    pub name: String,
    /// The account's email address.
    pub email: String,
}

/// Describes a session and its identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionView {
    /// The id of an agent or account (or bot).
    pub id: UserId,
    /// The name-in-use at the time this view was captured.
    pub name: String,
    /// The id of the server that captured this view.
    pub server_id: String,
    /// The era of the server that captured this view.
    pub server_era: String,
    /// Id of the session, unique across all sessions globally.
    pub session_id: String,
    /// If true, this session belongs to a member of staff.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_staff: bool,
    /// If true, this session belongs to a manager of the room.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_manager: bool,
    /// For hosts and staff, the virtual address of the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_address: Option<String>,
    /// For staff, the real address of the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_client_address: Option<String>,
}

/// A 13-character string, usually used as aunique identifier for some type of object.
///
/// It is the base-36 encoding of an unsigned, 64-bit integer.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

impl Snowflake {
    /// Maximum possible snowflake that can be safely handled by all of cove's
    /// parts.
    ///
    /// In theory, euphoria's snowflakes are 64-bit values and can take
    /// advantage of the full range. However, sqlite always stores integers as
    /// signed, and uses a maximum of 8 bytes (64 bits). Because of this, using
    /// [`u64::MAX`] here would lead to errors in some database interactions.
    ///
    /// For this reason, I'm limiting snowflakes to the range from `0` to
    /// [`i64::MAX`]. The euphoria backend isn't likely to change its
    /// representation of message ids to suddenly use the upper parts of the
    /// range, and since message ids mostly consist of a timestamp, this
    /// approach should last until at least 2075.
    pub const MAX: Self = Snowflake(i64::MAX as u64);
}

impl Serialize for Snowflake {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Convert u64 to base36 string
        let mut n = self.0;
        let mut result = String::with_capacity(13);
        for _ in 0..13 {
            let c = char::from_digit((n % 36) as u32, 36).unwrap();
            result.insert(0, c);
            n /= 36;
        }
        result.serialize(serializer)
    }
}

struct SnowflakeVisitor;

impl de::Visitor<'_> for SnowflakeVisitor {
    type Value = Snowflake;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a base36 string of length 13")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        // Convert base36 string to u64
        if v.len() != 13 {
            return Err(E::invalid_length(v.len(), &self));
        }
        let n = u64::from_str_radix(v, 36)
            .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))?;
        Ok(Snowflake(n))
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(SnowflakeVisitor)
    }
}

/// Time is specified as a signed 64-bit integer, giving the number of seconds
/// since the Unix Epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Time(#[serde(with = "time::serde::timestamp")] pub OffsetDateTime);

impl Time {
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc().replace_millisecond(0).unwrap())
    }
}

/// Identifies a user.
///
/// The prefix of this value (up to the colon) indicates a type of session,
/// while the suffix is a unique value for that type of session.
///
/// It is possible for this value to have no prefix and colon, and there is no
/// fixed format for the unique value.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserId(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum SessionType {
    Agent,
    Account,
    Bot,
}

impl UserId {
    pub fn session_type(&self) -> Option<SessionType> {
        if self.0.starts_with("agent:") {
            Some(SessionType::Agent)
        } else if self.0.starts_with("account:") {
            Some(SessionType::Account)
        } else if self.0.starts_with("bot:") {
            Some(SessionType::Bot)
        } else {
            None
        }
    }
}
