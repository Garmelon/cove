use cove_input::{KeyBinding, KeyGroup, KeyGroupInfo};
use serde::Deserialize;

use crate::doc::Document;

macro_rules! default_bindings {
    ( $(
        pub mod $mod:ident { $(
            pub fn $name:ident => [ $($key:expr),* ];
        )* }
    )*) => {
        mod default { $(
            pub mod $mod { $(
                pub fn $name() -> ::cove_input::KeyBinding {
                    ::cove_input::KeyBinding::new().with_keys([ $($key),* ]).unwrap()
                }
            )* }
        )* }
    };
}

default_bindings! {
    pub mod general {
        pub fn exit => ["ctrl+c"];
        pub fn abort => ["esc"];
        pub fn confirm => ["enter"];
        pub fn focus => ["tab"];
        pub fn help => ["f1"];
        pub fn log => ["f12"];
    }

    pub mod scroll {
        pub fn up_line => ["ctrl+y"];
        pub fn down_line => ["ctrl+e"];
        pub fn up_half => ["ctrl+u"];
        pub fn down_half => ["ctrl+d"];
        pub fn up_full => ["ctrl+b", "pageup"];
        pub fn down_full => ["ctrl+f", "pagedown"];
        pub fn center_cursor => ["z"];
    }

    pub mod cursor {
        pub fn up => ["k", "up"];
        pub fn down => ["j", "down"];
        pub fn to_top => ["g", "home"];
        pub fn to_bottom => ["G", "end"];
    }

    pub mod editor_cursor {
        pub fn left => ["ctrl+b","left"];
        pub fn right => ["ctrl+f", "right"];
        pub fn left_word => ["alt+b", "ctrl+left"];
        pub fn right_word => ["alt+f", "ctrl+right"];
        pub fn start => ["ctrl+a", "home"];
        pub fn end => ["ctrl+e", "end"];
        pub fn up => ["up"];
        pub fn down => ["down"];
    }

    pub mod editor_action {
        pub fn backspace => ["ctrl+h", "backspace"];
        pub fn delete => ["ctrl+d", "delete"];
        pub fn clear => ["ctrl+l"];
        pub fn external => ["ctrl+x", "alt+e"];
    }

    pub mod rooms_action {
        pub fn connect => ["c"];
        pub fn connect_all => ["C"];
        pub fn disconnect => ["d"];
        pub fn disconnect_all => ["D"];
        pub fn connect_autojoin => ["a"];
        pub fn disconnect_non_autojoin => ["A"];
        pub fn new => ["n"];
        pub fn delete => ["X"];
        pub fn change_sort_order => ["s"];
    }

    pub mod room_action {
        pub fn authenticate => ["a"];
        pub fn nick => ["n"];
        pub fn more_messages => ["m"];
        pub fn account => ["A"];
    }

    pub mod tree_cursor {
        pub fn to_above_sibling => ["K", "ctrl+up"];
        pub fn to_below_sibling => ["J", "ctrl+down"];
        pub fn to_parent => ["p"];
        pub fn to_root => ["P"];
        pub fn to_older_message => ["h", "left"];
        pub fn to_newer_message => ["l", "right"];
        pub fn to_older_unseen_message => ["H", "ctrl+left"];
        pub fn to_newer_unseen_message => ["L", "ctrl+right"];
    }

    pub mod tree_action {
        pub fn reply => ["r"];
        pub fn reply_alternate => ["R"];
        pub fn new_thread => ["t"];
        pub fn fold_tree => [" "];
        pub fn toggle_seen => ["s"];
        pub fn mark_visible_seen => ["S"];
        pub fn mark_older_seen => ["ctrl+s"];
        pub fn info => ["i"];
        pub fn links => ["I"];
        pub fn toggle_nick_emoji => ["e"];
        pub fn increase_caesar => ["c"];
        pub fn decrease_caesar => ["C"];
    }

}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// General.
pub struct General {
    /// Quit cove.
    #[serde(default = "default::general::exit")]
    pub exit: KeyBinding,
    /// Abort/close.
    #[serde(default = "default::general::abort")]
    pub abort: KeyBinding,
    /// Confirm.
    #[serde(default = "default::general::confirm")]
    pub confirm: KeyBinding,
    /// Advance focus.
    #[serde(default = "default::general::focus")]
    pub focus: KeyBinding,
    /// Show this help.
    #[serde(default = "default::general::help")]
    pub help: KeyBinding,
    /// Show log.
    #[serde(default = "default::general::log")]
    pub log: KeyBinding,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Scrolling.
pub struct Scroll {
    /// Scroll up one line.
    #[serde(default = "default::scroll::up_line")]
    pub up_line: KeyBinding,
    /// Scroll down one line.
    #[serde(default = "default::scroll::down_line")]
    pub down_line: KeyBinding,
    /// Scroll up half a screen.
    #[serde(default = "default::scroll::up_half")]
    pub up_half: KeyBinding,
    /// Scroll down half a screen.
    #[serde(default = "default::scroll::down_half")]
    pub down_half: KeyBinding,
    /// Scroll up a full screen.
    #[serde(default = "default::scroll::up_full")]
    pub up_full: KeyBinding,
    /// Scroll down a full screen.
    #[serde(default = "default::scroll::down_full")]
    pub down_full: KeyBinding,
    /// Center cursor.
    #[serde(default = "default::scroll::center_cursor")]
    pub center_cursor: KeyBinding,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Cursor movement.
pub struct Cursor {
    /// Move up.
    #[serde(default = "default::cursor::up")]
    pub up: KeyBinding,
    /// Move down.
    #[serde(default = "default::cursor::down")]
    pub down: KeyBinding,
    /// Move to top.
    #[serde(default = "default::cursor::to_top")]
    pub to_top: KeyBinding,
    /// Move to bottom.
    #[serde(default = "default::cursor::to_bottom")]
    pub to_bottom: KeyBinding,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Editor cursor movement.
pub struct EditorCursor {
    /// Move left.
    #[serde(default = "default::editor_cursor::left")]
    pub left: KeyBinding,
    /// Move right.
    #[serde(default = "default::editor_cursor::right")]
    pub right: KeyBinding,
    /// Move left a word.
    #[serde(default = "default::editor_cursor::left_word")]
    pub left_word: KeyBinding,
    /// Move right a word.
    #[serde(default = "default::editor_cursor::right_word")]
    pub right_word: KeyBinding,
    /// Move to start of line.
    #[serde(default = "default::editor_cursor::start")]
    pub start: KeyBinding,
    /// Move to end of line.
    #[serde(default = "default::editor_cursor::end")]
    pub end: KeyBinding,
    /// Move up.
    #[serde(default = "default::editor_cursor::up")]
    pub up: KeyBinding,
    /// Move down.
    #[serde(default = "default::editor_cursor::down")]
    pub down: KeyBinding,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Editor actions.
pub struct EditorAction {
    /// Delete before cursor.
    #[serde(default = "default::editor_action::backspace")]
    pub backspace: KeyBinding,
    /// Delete after cursor.
    #[serde(default = "default::editor_action::delete")]
    pub delete: KeyBinding,
    /// Clear editor contents.
    #[serde(default = "default::editor_action::clear")]
    pub clear: KeyBinding,
    /// Edit in external editor.
    #[serde(default = "default::editor_action::external")]
    pub external: KeyBinding,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Editor {
    #[serde(default)]
    #[document(no_default)]
    pub cursor: EditorCursor,

    #[serde(default)]
    #[document(no_default)]
    pub action: EditorAction,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Room list actions.
pub struct RoomsAction {
    /// Connect to selected room.
    #[serde(default = "default::rooms_action::connect")]
    pub connect: KeyBinding,
    /// Connect to all rooms.
    #[serde(default = "default::rooms_action::connect_all")]
    pub connect_all: KeyBinding,
    /// Disconnect from selected room.
    #[serde(default = "default::rooms_action::disconnect")]
    pub disconnect: KeyBinding,
    /// Disconnect from all rooms.
    #[serde(default = "default::rooms_action::disconnect_all")]
    pub disconnect_all: KeyBinding,
    /// Connect to all autojoin rooms.
    #[serde(default = "default::rooms_action::connect_autojoin")]
    pub connect_autojoin: KeyBinding,
    /// Disconnect from all non-autojoin rooms.
    #[serde(default = "default::rooms_action::disconnect_non_autojoin")]
    pub disconnect_non_autojoin: KeyBinding,
    /// Connect to new room.
    #[serde(default = "default::rooms_action::new")]
    pub new: KeyBinding,
    /// Delete room.
    #[serde(default = "default::rooms_action::delete")]
    pub delete: KeyBinding,
    /// Change sort order.
    #[serde(default = "default::rooms_action::change_sort_order")]
    pub change_sort_order: KeyBinding,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Rooms {
    #[serde(default)]
    #[document(no_default)]
    pub action: RoomsAction,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Room actions.
pub struct RoomAction {
    /// Authenticate.
    #[serde(default = "default::room_action::authenticate")]
    pub authenticate: KeyBinding,
    /// Change nick.
    #[serde(default = "default::room_action::nick")]
    pub nick: KeyBinding,
    /// Download more messages.
    #[serde(default = "default::room_action::more_messages")]
    pub more_messages: KeyBinding,
    /// Manage account.
    #[serde(default = "default::room_action::account")]
    pub account: KeyBinding,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Room {
    #[serde(default)]
    #[document(no_default)]
    pub action: RoomAction,
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Tree cursor movement.
pub struct TreeCursor {
    /// Move to above sibling.
    #[serde(default = "default::tree_cursor::to_above_sibling")]
    pub to_above_sibling: KeyBinding,
    /// Move to below sibling.
    #[serde(default = "default::tree_cursor::to_below_sibling")]
    pub to_below_sibling: KeyBinding,
    /// Move to parent.
    #[serde(default = "default::tree_cursor::to_parent")]
    pub to_parent: KeyBinding,
    /// Move to root.
    #[serde(default = "default::tree_cursor::to_root")]
    pub to_root: KeyBinding,
    /// Move to older message.
    #[serde(default = "default::tree_cursor::to_older_message")]
    pub to_older_message: KeyBinding,
    /// Move to newer message.
    #[serde(default = "default::tree_cursor::to_newer_message")]
    pub to_newer_message: KeyBinding,
    /// Move to older unseen message.
    #[serde(default = "default::tree_cursor::to_older_unseen_message")]
    pub to_older_unseen_message: KeyBinding,
    /// Move to newer unseen message.
    #[serde(default = "default::tree_cursor::to_newer_unseen_message")]
    pub to_newer_unseen_message: KeyBinding,
    // TODO Bindings inspired by vim's ()/[]/{} bindings?
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
/// Tree actions.
pub struct TreeAction {
    /// Reply to message, inline if possible.
    #[serde(default = "default::tree_action::reply")]
    pub reply: KeyBinding,
    /// Reply opposite to normal reply.
    #[serde(default = "default::tree_action::reply_alternate")]
    pub reply_alternate: KeyBinding,
    /// Start a new thread.
    #[serde(default = "default::tree_action::new_thread")]
    pub new_thread: KeyBinding,
    /// Fold current message's subtree.
    #[serde(default = "default::tree_action::fold_tree")]
    pub fold_tree: KeyBinding,
    /// Toggle current message's seen status.
    #[serde(default = "default::tree_action::toggle_seen")]
    pub toggle_seen: KeyBinding,
    /// Mark all visible messages as seen.
    #[serde(default = "default::tree_action::mark_visible_seen")]
    pub mark_visible_seen: KeyBinding,
    /// Mark all older messages as seen.
    #[serde(default = "default::tree_action::mark_older_seen")]
    pub mark_older_seen: KeyBinding,
    /// Inspect selected element.
    #[serde(default = "default::tree_action::info")]
    pub inspect: KeyBinding,
    /// List links found in message.
    #[serde(default = "default::tree_action::links")]
    pub links: KeyBinding,
    /// Toggle agent id based nick emoji.
    #[serde(default = "default::tree_action::toggle_nick_emoji")]
    pub toggle_nick_emoji: KeyBinding,
    /// Increase caesar cipher rotation.
    #[serde(default = "default::tree_action::increase_caesar")]
    pub increase_caesar: KeyBinding,
    /// Decrease caesar cipher rotation.
    #[serde(default = "default::tree_action::decrease_caesar")]
    pub decrease_caesar: KeyBinding,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Tree {
    #[serde(default)]
    #[document(no_default)]
    pub cursor: TreeCursor,

    #[serde(default)]
    #[document(no_default)]
    pub action: TreeAction,
}

#[derive(Debug, Default, Deserialize, Document)]
pub struct Keys {
    #[serde(default)]
    #[document(no_default)]
    pub general: General,

    #[serde(default)]
    #[document(no_default)]
    pub scroll: Scroll,

    #[serde(default)]
    #[document(no_default)]
    pub cursor: Cursor,

    #[serde(default)]
    #[document(no_default)]
    pub editor: Editor,

    #[serde(default)]
    #[document(no_default)]
    pub rooms: Rooms,

    #[serde(default)]
    #[document(no_default)]
    pub room: Room,

    #[serde(default)]
    #[document(no_default)]
    pub tree: Tree,
}

impl Keys {
    pub fn groups(&self) -> Vec<KeyGroupInfo<'_>> {
        vec![
            KeyGroupInfo::new("general", &self.general),
            KeyGroupInfo::new("scroll", &self.scroll),
            KeyGroupInfo::new("cursor", &self.cursor),
            KeyGroupInfo::new("editor.cursor", &self.editor.cursor),
            KeyGroupInfo::new("editor.action", &self.editor.action),
            KeyGroupInfo::new("rooms.action", &self.rooms.action),
            KeyGroupInfo::new("room.action", &self.room.action),
            KeyGroupInfo::new("tree.cursor", &self.tree.cursor),
            KeyGroupInfo::new("tree.action", &self.tree.action),
        ]
    }
}
