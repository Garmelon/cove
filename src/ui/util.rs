use std::sync::Arc;

use crossterm::event::KeyCode;
use parking_lot::FairMutex;
use toss::terminal::Terminal;

use super::input::{key, KeyBindingsList, KeyEvent};
use super::widgets::editor::EditorState;

pub fn prompt(
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    initial_text: &str,
) -> Option<String> {
    let content = {
        let _guard = crossterm_lock.lock();
        terminal.suspend().expect("could not suspend");
        let content = edit::edit(initial_text);
        terminal.unsuspend().expect("could not unsuspend");
        content
    };

    // TODO Don't swipe this error under the rug
    let content = content.ok()?;

    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

// TODO Support more of the emacs-y bindings, see bash as example

pub fn list_editor_key_bindings(
    bindings: &mut KeyBindingsList,
    char_filter: impl Fn(char) -> bool,
    can_edit_externally: bool,
) {
    if char_filter('\n') {
        bindings.binding("enter+<any modifier>", "insert newline");
    }

    // Editing
    bindings.binding("ctrl+h, backspace", "delete before cursor");
    bindings.binding("ctrl+d, delete", "delete after cursor");
    bindings.binding("ctrl+l", "clear editor contents");
    if can_edit_externally {
        bindings.binding("ctrl+e", "edit in $EDITOR");
    }

    bindings.empty();

    // Cursor movement
    bindings.binding("ctrl+b, ←", "move cursor left");
    bindings.binding("ctrl+f, →", "move cursor right");
    bindings.binding("alt+b, ctrl+←", "move cursor left a word");
    bindings.binding("alt+f, ctrl+→", "move cursor right a word");
    bindings.binding("ctrl+a, home", "move cursor to start of line");
    bindings.binding("ctrl+e, end", "move cursor to end of line");
    bindings.binding("↑/↓", "move cursor up/down");
}

pub fn handle_editor_key_event(
    editor: &EditorState,
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    event: KeyEvent,
    char_filter: impl Fn(char) -> bool,
    can_edit_externally: bool,
) -> bool {
    match event {
        // Enter with *any* modifier pressed - if ctrl and shift don't
        // work, maybe alt does
        key!(Enter) => return false,
        KeyEvent {
            code: KeyCode::Enter,
            ..
        } if char_filter('\n') => editor.insert_char(terminal.frame(), '\n'),

        // Editing
        key!(Char ch) if char_filter(ch) => editor.insert_char(terminal.frame(), ch),
        key!(Ctrl + 'h') | key!(Backspace) => editor.backspace(terminal.frame()),
        key!(Ctrl + 'd') | key!(Delete) => editor.delete(),
        key!(Ctrl + 'l') => editor.clear(),
        key!(Ctrl + 'e') if can_edit_externally => editor.edit_externally(terminal, crossterm_lock), // TODO Change to some other binding

        // Cursor movement
        key!(Ctrl + 'b') | key!(Left) => editor.move_cursor_left(terminal.frame()),
        key!(Ctrl + 'f') | key!(Right) => editor.move_cursor_right(terminal.frame()),
        key!(Alt + 'b') | key!(Ctrl + Left) => editor.move_cursor_left_a_word(terminal.frame()),
        key!(Alt + 'f') | key!(Ctrl + Right) => editor.move_cursor_right_a_word(terminal.frame()),
        key!(Ctrl + 'a') | key!(Home) => editor.move_cursor_to_start_of_line(terminal.frame()),
        key!(Ctrl + 'e') | key!(End) => editor.move_cursor_to_end_of_line(terminal.frame()),
        key!(Up) => editor.move_cursor_up(terminal.frame()),
        key!(Down) => editor.move_cursor_down(terminal.frame()),

        _ => return false,
    }

    true
}
