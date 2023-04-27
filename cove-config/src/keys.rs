use cove_input::{Group, KeyBinding};

#[derive(Debug, Group)]
pub struct General {
    /// Quit cove.
    pub exit: KeyBinding,
    /// Abort/close.
    pub abort: KeyBinding,
    /// Confirm.
    pub confirm: KeyBinding,
    /// Show this help.
    pub help: KeyBinding,
    /// Show log.
    pub log: KeyBinding,
}

#[derive(Debug, Group)]
pub struct Scroll {
    /// Scroll up one line.
    pub up_line: KeyBinding,
    /// Scroll down one line.
    pub down_line: KeyBinding,
    /// Scroll up half a screen.
    pub up_half: KeyBinding,
    /// Scroll down half a screen.
    pub down_half: KeyBinding,
    /// Scroll up a full screen.
    pub up_full: KeyBinding,
    /// Scroll down a full screen.
    pub down_full: KeyBinding,
}

#[derive(Debug, Group)]
pub struct Cursor {
    /// Move cursor up.
    pub up: KeyBinding,
    /// Move cursor down.
    pub down: KeyBinding,
    /// Move cursor to top.
    pub to_top: KeyBinding,
    /// Move cursor to bottom.
    pub to_bottom: KeyBinding,
    /// Center cursor.
    pub center: KeyBinding,
}

#[derive(Debug, Group)]
pub struct TreeCursor {
    /// Move cursor to above sibling.
    pub to_above_sibling: KeyBinding,
    /// Move cursor to below sibling.
    pub to_below_sibling: KeyBinding,
    /// Move cursor to parent.
    pub to_parent: KeyBinding,
    /// Move cursor to root.
    pub to_root: KeyBinding,
    /// Move cursor to previous message.
    pub to_prev_message: KeyBinding,
    /// Move cursor to next message.
    pub to_next_message: KeyBinding,
}

#[derive(Debug, Group)]
pub struct EditorCursor {
    /// Move cursor left.
    pub left: KeyBinding,
    /// Move cursor right.
    pub right: KeyBinding,
    /// Move cursor left a word.
    pub left_word: KeyBinding,
    /// Move cursor right a word.
    pub right_word: KeyBinding,
    /// Move cursor to start of line.
    pub start: KeyBinding,
    /// Move cursor to end of line.
    pub end: KeyBinding,
    /// Move cursor up.
    pub up: KeyBinding,
    /// Move cursor down.
    pub down: KeyBinding,
}

#[derive(Debug, Group)]
pub struct EditorOp {
    /// Insert newline.
    pub newline: KeyBinding,
    /// Delete before cursor.
    pub backspace: KeyBinding,
    /// Delete after cursor.
    pub delete: KeyBinding,
    /// Clear editor contents.
    pub clear: KeyBinding,
}
