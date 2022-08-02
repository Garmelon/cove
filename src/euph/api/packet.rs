use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::PacketType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub id: Option<String>,
    pub r#type: PacketType,
    pub data: Option<Value>,
    #[serde(skip_serializing)]
    pub error: Option<String>,
    #[serde(default, skip_serializing)]
    pub throttled: bool,
    #[serde(skip_serializing)]
    pub throttled_reason: Option<String>,
}

pub trait Command {
    type Reply;
}

macro_rules! packets {
    ( $( $name:ident, )*) => {
        #[derive(Debug, Clone)]
        #[non_exhaustive]
        pub enum Data {
            $( $name(super::$name), )*
            Unimplemented,
        }

        impl Data {
            pub fn from_value(ptype: PacketType, value: Value) -> serde_json::Result<Self> {
                Ok(match ptype {
                    $( PacketType::$name => Self::$name(serde_json::from_value(value)?), )*
                    _ => Self::Unimplemented,
                })
            }

            pub fn into_value(self) -> serde_json::Result<Value> {
                Ok(match self{
                    $( Self::$name(p) => serde_json::to_value(p)?, )*
                    Self::Unimplemented => panic!("using unimplemented data"),
                })
            }

            pub fn packet_type(&self) -> PacketType {
                match self {
                    $( Self::$name(_) => PacketType::$name, )*
                    Self::Unimplemented => panic!("using unimplemented data"),
                }
            }
        }

        $(
            impl From<super::$name> for Data {
                fn from(p: super::$name) -> Self {
                    Self::$name(p)
                }
            }

            impl TryFrom<Data> for super::$name{
                type Error = ();

                fn try_from(value: Data) -> Result<Self, Self::Error> {
                    match value {
                        Data::$name(p) => Ok(p),
                        _ => Err(())
                    }
                }
            }
        )*
    };
}

macro_rules! commands {
    ( $( $cmd:ident => $rpl:ident, )* ) => {
        $(
            impl Command for super::$cmd {
                type Reply = super::$rpl;
            }
        )*
    };
}

packets! {
    BounceEvent,
    DisconnectEvent,
    HelloEvent,
    JoinEvent,
    LoginEvent,
    LogoutEvent,
    NetworkEvent,
    NickEvent,
    EditMessageEvent,
    PartEvent,
    PingEvent,
    PmInitiateEvent,
    SendEvent,
    SnapshotEvent,
    Auth,
    AuthReply,
    Ping,
    PingReply,
    GetMessage,
    GetMessageReply,
    Log,
    LogReply,
    Nick,
    NickReply,
    PmInitiate,
    PmInitiateReply,
    Send,
    SendReply,
    Who,
    WhoReply,
}

commands! {
    Auth => AuthReply,
    Ping => PingReply,
    GetMessage => GetMessageReply,
    Log => LogReply,
    Nick => NickReply,
    PmInitiate => PmInitiateReply,
    Send => SendReply,
    Who => WhoReply,
}

#[derive(Debug, Clone)]
pub struct ParsedPacket {
    pub id: Option<String>,
    pub r#type: PacketType,
    pub content: Result<Data, String>,
    pub throttled: Option<String>,
}

impl ParsedPacket {
    pub fn from_packet(packet: Packet) -> serde_json::Result<Self> {
        let id = packet.id;
        let r#type = packet.r#type;

        let content = if let Some(error) = packet.error {
            Err(error)
        } else {
            let data = packet.data.unwrap_or_default();
            Ok(Data::from_value(r#type, data)?)
        };

        let throttled = if packet.throttled {
            let reason = packet
                .throttled_reason
                .unwrap_or_else(|| "no reason given".to_string());
            Some(reason)
        } else {
            None
        };

        Ok(Self {
            id,
            r#type,
            content,
            throttled,
        })
    }

    pub fn into_packet(self) -> serde_json::Result<Packet> {
        let id = self.id;
        let r#type = self.r#type;
        let throttled = self.throttled.is_some();
        let throttled_reason = self.throttled;

        Ok(match self.content {
            Ok(data) => Packet {
                id,
                r#type,
                data: Some(data.into_value()?),
                error: None,
                throttled,
                throttled_reason,
            },
            Err(error) => Packet {
                id,
                r#type,
                data: None,
                error: Some(error),
                throttled,
                throttled_reason,
            },
        })
    }
}
