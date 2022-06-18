//! Models the euphoria API at <http://api.euphoria.io/>.

mod events;
mod room_cmds;
mod session_cmds;
mod types;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use events::*;
pub use room_cmds::*;
pub use session_cmds::*;
pub use types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub id: Option<String>,
    pub r#type: PacketType,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub throttled: Option<bool>,
    pub throttled_reason: Option<String>,
}
