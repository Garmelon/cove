//! Session commands.

use serde::{Deserialize, Serialize};

use super::{has_packet_type, AuthOption, HasPacketType, PacketType, Time};

/// Attempt to join a private room.
///
/// This should be sent in response to a bounce event at the beginning of a
/// session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    /// The method of authentication.
    pub r#type: AuthOption,
    /// Use this field for [`AuthOption::Passcode`] authentication.
    pub passcode: Option<String>,
}

has_packet_type!(Auth);

/// Reports whether the [`Auth`] command succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthReply {
    /// True if authentication succeeded.
    pub success: bool,
    /// If [`Self::success`] was false, the reason for failure.
    pub reason: Option<String>,
}

has_packet_type!(AuthReply);

/// Initiate a client-to-server ping.
///
/// The server will send back a [`PingReply`] with the same timestamp as soon as
/// possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    /// An arbitrary value, intended to be a unix timestamp.
    pub time: Time,
}

has_packet_type!(Ping);

/// Response to a [`Ping`] command or [`PingEvent`](super::PingEvent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingReply {
    /// The timestamp of the ping being replied to.
    pub time: Option<Time>,
}

has_packet_type!(PingReply);
