mod switch_room;

pub use switch_room::*;

use super::RoomId;

pub enum Overlay {
    SwitchRoom(SwitchRoomState),
}

pub enum OverlayReaction {
    Handled,
    Close,
    SwitchRoom(RoomId),
}
