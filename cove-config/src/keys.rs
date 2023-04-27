use cove_input::{KeyBinding, KeyGroup};
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
        pub fn help => ["f1", "?"];
        pub fn log => ["f12"];
    }

    pub mod scroll {
        pub fn up_line => ["ctrl+y"];
        pub fn down_line => ["ctrl+e"];
        pub fn up_half => ["ctrl+u"];
        pub fn down_half => ["ctrl+d"];
        pub fn up_full => ["ctrl+b"];
        pub fn down_full => ["ctrl+f"];
    }

    pub mod cursor {
        pub fn up => ["k", "up"];
        pub fn down => ["j", "down"];
        pub fn to_top => ["g", "home"];
        pub fn to_bottom => ["G", "end"];
        pub fn center => ["z"];
    }

    pub mod tree_cursor {
        pub fn to_above_sibling => ["K", "ctrl+up"];
        pub fn to_below_sibling => ["J", "ctrl+down"];
        pub fn to_parent => ["p"];
        pub fn to_root => ["P"];
        pub fn to_prev_message => ["h", "left"];
        pub fn to_next_message => ["l", "right"];
        pub fn to_prev_unseen_message => ["H", "ctrl+left"];
        pub fn to_next_unseen_message => ["L", "ctrl+right"];
    }

    pub mod tree_action {
        pub fn reply => ["r"];
        pub fn reply_alternate => ["R"];
        pub fn new_thread => ["t"];
        pub fn fold_tree => [" "];
        pub fn toggle_seen => ["s"];
        pub fn mark_visible_seen => ["S"];
        pub fn mark_older_seen => ["ctrl+s"];
    }

    pub mod editor_cursor {
        pub fn left => [];
        pub fn right => [];
        pub fn left_word => [];
        pub fn right_word => [];
        pub fn start => [];
        pub fn end => [];
        pub fn up => [];
        pub fn down => [];
    }

    pub mod editor_action {
        pub fn newline => [];
        pub fn backspace => [];
        pub fn delete => [];
        pub fn clear => [];
    }

}

#[derive(Debug, Deserialize, Document, KeyGroup)]
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
    /// Show this help.
    #[serde(default = "default::general::help")]
    pub help: KeyBinding,
    /// Show log.
    #[serde(default = "default::general::log")]
    pub log: KeyBinding,
}

impl Default for General {
    fn default() -> Self {
        Self {
            exit: default::general::exit(),
            abort: default::general::abort(),
            confirm: default::general::confirm(),
            help: default::general::help(),
            log: default::general::log(),
        }
    }
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
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
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            up_line: default::scroll::up_line(),
            down_line: default::scroll::down_line(),
            up_half: default::scroll::up_half(),
            down_half: default::scroll::down_half(),
            up_full: default::scroll::up_full(),
            down_full: default::scroll::down_full(),
        }
    }
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
pub struct Cursor {
    /// Move cursor up.
    #[serde(default = "default::cursor::up")]
    pub up: KeyBinding,
    /// Move cursor down.
    #[serde(default = "default::cursor::down")]
    pub down: KeyBinding,
    /// Move cursor to top.
    #[serde(default = "default::cursor::to_top")]
    pub to_top: KeyBinding,
    /// Move cursor to bottom.
    #[serde(default = "default::cursor::to_bottom")]
    pub to_bottom: KeyBinding,
    /// Center cursor.
    #[serde(default = "default::cursor::center")]
    pub center: KeyBinding,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            up: default::cursor::up(),
            down: default::cursor::down(),
            to_top: default::cursor::to_top(),
            to_bottom: default::cursor::to_bottom(),
            center: default::cursor::center(),
        }
    }
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
pub struct EditorCursor {
    /// Move cursor left.
    #[serde(default = "default::editor_cursor::left")]
    pub left: KeyBinding,
    /// Move cursor right.
    #[serde(default = "default::editor_cursor::right")]
    pub right: KeyBinding,
    /// Move cursor left a word.
    #[serde(default = "default::editor_cursor::left_word")]
    pub left_word: KeyBinding,
    /// Move cursor right a word.
    #[serde(default = "default::editor_cursor::right_word")]
    pub right_word: KeyBinding,
    /// Move cursor to start of line.
    #[serde(default = "default::editor_cursor::start")]
    pub start: KeyBinding,
    /// Move cursor to end of line.
    #[serde(default = "default::editor_cursor::end")]
    pub end: KeyBinding,
    /// Move cursor up.
    #[serde(default = "default::editor_cursor::up")]
    pub up: KeyBinding,
    /// Move cursor down.
    #[serde(default = "default::editor_cursor::down")]
    pub down: KeyBinding,
}

impl Default for EditorCursor {
    fn default() -> Self {
        Self {
            left: default::editor_cursor::left(),
            right: default::editor_cursor::right(),
            left_word: default::editor_cursor::left_word(),
            right_word: default::editor_cursor::right_word(),
            start: default::editor_cursor::start(),
            end: default::editor_cursor::end(),
            up: default::editor_cursor::up(),
            down: default::editor_cursor::down(),
        }
    }
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
pub struct EditorAction {
    /// Insert newline.
    #[serde(default = "default::editor_action::newline")]
    pub newline: KeyBinding,
    /// Delete before cursor.
    #[serde(default = "default::editor_action::backspace")]
    pub backspace: KeyBinding,
    /// Delete after cursor.
    #[serde(default = "default::editor_action::delete")]
    pub delete: KeyBinding,
    /// Clear editor contents.
    #[serde(default = "default::editor_action::clear")]
    pub clear: KeyBinding,
}

impl Default for EditorAction {
    fn default() -> Self {
        Self {
            newline: default::editor_action::newline(),
            backspace: default::editor_action::backspace(),
            delete: default::editor_action::delete(),
            clear: default::editor_action::clear(),
        }
    }
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
pub struct TreeCursor {
    /// Move cursor to above sibling.
    #[serde(default = "default::tree_cursor::to_above_sibling")]
    pub to_above_sibling: KeyBinding,
    /// Move cursor to below sibling.
    #[serde(default = "default::tree_cursor::to_below_sibling")]
    pub to_below_sibling: KeyBinding,
    /// Move cursor to parent.
    #[serde(default = "default::tree_cursor::to_parent")]
    pub to_parent: KeyBinding,
    /// Move cursor to root.
    #[serde(default = "default::tree_cursor::to_root")]
    pub to_root: KeyBinding,
    /// Move cursor to previous message.
    #[serde(default = "default::tree_cursor::to_prev_message")]
    pub to_prev_message: KeyBinding,
    /// Move cursor to next message.
    #[serde(default = "default::tree_cursor::to_next_message")]
    pub to_next_message: KeyBinding,
    /// Move cursor to previous unseen message.
    #[serde(default = "default::tree_cursor::to_prev_unseen_message")]
    pub to_prev_unseen_message: KeyBinding,
    /// Move cursor to next unseen message.
    #[serde(default = "default::tree_cursor::to_next_unseen_message")]
    pub to_next_unseen_message: KeyBinding,
}

impl Default for TreeCursor {
    fn default() -> Self {
        Self {
            to_above_sibling: default::tree_cursor::to_above_sibling(),
            to_below_sibling: default::tree_cursor::to_below_sibling(),
            to_parent: default::tree_cursor::to_parent(),
            to_root: default::tree_cursor::to_root(),
            to_prev_message: default::tree_cursor::to_prev_message(),
            to_next_message: default::tree_cursor::to_next_message(),
            to_prev_unseen_message: default::tree_cursor::to_prev_unseen_message(),
            to_next_unseen_message: default::tree_cursor::to_next_unseen_message(),
        }
    }
}

#[derive(Debug, Deserialize, Document, KeyGroup)]
pub struct TreeAction {
    /// Reply to message (inline if possible).
    #[serde(default = "default::tree_action::reply")]
    pub reply: KeyBinding,
    /// Reply to message, opposite of normal reply.
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
}

impl Default for TreeAction {
    fn default() -> Self {
        Self {
            reply: default::tree_action::reply(),
            reply_alternate: default::tree_action::reply_alternate(),
            new_thread: default::tree_action::new_thread(),
            fold_tree: default::tree_action::fold_tree(),
            toggle_seen: default::tree_action::toggle_seen(),
            mark_visible_seen: default::tree_action::mark_visible_seen(),
            mark_older_seen: default::tree_action::mark_older_seen(),
        }
    }
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
    pub tree: Tree,
}
