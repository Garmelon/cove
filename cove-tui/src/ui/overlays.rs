mod switch_room;

pub use switch_room::*;

pub enum Overlay {
    SwitchRoom(SwitchRoomState),
}

pub enum OverlayReaction {
    Handled,
    Close,
    SwitchRoom(String),
}
