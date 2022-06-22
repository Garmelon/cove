use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error("throttled: {0}")]
    Throttled(String),
    #[error("error: {0}")]
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub id: Option<String>,
    pub r#type: PacketType,
    pub data: Option<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub throttled: bool,
    pub throttled_reason: Option<String>,
}

impl Packet {
    pub fn data(self) -> Result<Value, PacketError> {
        if self.throttled {
            let reason = self
                .throttled_reason
                .unwrap_or_else(|| "no reason given".to_string());
            Err(PacketError::Throttled(reason))
        } else if let Some(error) = self.error {
            Err(PacketError::Error(error))
        } else {
            Ok(self.data.unwrap_or_default())
        }
    }
}

pub trait HasPacketType {
    fn packet_type() -> PacketType;
}

macro_rules! has_packet_type {
    ($name:ident) => {
        impl HasPacketType for $name {
            fn packet_type() -> PacketType {
                PacketType::$name
            }
        }
    };
}
pub(crate) use has_packet_type;

pub trait ToPacket {
    fn to_packet(self, id: Option<String>) -> Packet;
}

impl<T: HasPacketType + Serialize> ToPacket for T {
    fn to_packet(self, id: Option<String>) -> Packet {
        Packet {
            id,
            r#type: Self::packet_type(),
            data: Some(serde_json::to_value(self).expect("malformed packet")),
            error: None,
            throttled: false,
            throttled_reason: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("incorrect packet type: expected {expected}, got {actual}")]
    IncorrectType {
        expected: PacketType,
        actual: PacketType,
    },
    #[error("{0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("{0}")]
    Packet(#[from] PacketError),
}

pub trait FromPacket: Sized {
    fn from_packet(packet: Packet) -> Result<Self, DecodeError>;
}

impl<T: HasPacketType + DeserializeOwned> FromPacket for T {
    fn from_packet(packet: Packet) -> Result<Self, DecodeError> {
        if packet.r#type != Self::packet_type() {
            Err(DecodeError::IncorrectType {
                expected: Self::packet_type(),
                actual: packet.r#type,
            })
        } else {
            let data = packet.data()?;
            Ok(serde_json::from_value(data)?)
        }
    }
}
