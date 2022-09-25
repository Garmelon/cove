use std::io;
use std::sync::Arc;

use parking_lot::FairMutex;
use toss::terminal::Terminal;

use super::input::{key, InputEvent, KeyBindingsList};
use super::widgets::editor::EditorState;

pub fn prompt(
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    initial_text: &str,
) -> io::Result<String> {
    let content = {
        let _guard = crossterm_lock.lock();
        terminal.suspend().expect("could not suspend");
        let content = edit::edit(initial_text);
        terminal.unsuspend().expect("could not unsuspend");
        content
    };

    content
}

fn list_editor_editing_key_bindings(
    bindings: &mut KeyBindingsList,
    char_filter: impl Fn(char) -> bool,
) {
    if char_filter('\n') {
        bindings.binding("enter+<any modifier>", "insert newline");
    }

    bindings.binding("ctrl+h, backspace", "delete before cursor");
    bindings.binding("ctrl+d, delete", "delete after cursor");
    bindings.binding("ctrl+l", "clear editor contents");
}

fn list_editor_cursor_movement_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("ctrl+b, ←", "move cursor left");
    bindings.binding("ctrl+f, →", "move cursor right");
    bindings.binding("alt+b, ctrl+←", "move cursor left a word");
    bindings.binding("alt+f, ctrl+→", "move cursor right a word");
    bindings.binding("ctrl+a, home", "move cursor to start of line");
    bindings.binding("ctrl+e, end", "move cursor to end of line");
    bindings.binding("↑/↓", "move cursor up/down");
}

pub fn list_editor_key_bindings(
    bindings: &mut KeyBindingsList,
    char_filter: impl Fn(char) -> bool,
) {
    list_editor_editing_key_bindings(bindings, char_filter);
    bindings.empty();
    list_editor_cursor_movement_key_bindings(bindings);
}

pub fn handle_editor_input_event(
    editor: &EditorState,
    terminal: &mut Terminal,
    event: &InputEvent,
    char_filter: impl Fn(char) -> bool,
) -> bool {
    match event {
        // Enter with *any* modifier pressed - if ctrl and shift don't
        // work, maybe alt does
        key!(Enter) => return false,
        InputEvent::Key(crate::ui::input::KeyEvent {
            code: crossterm::event::KeyCode::Enter,
            ..
        }) if char_filter('\n') => editor.insert_char(terminal.frame(), '\n'),

        // Editing
        key!(Char ch) if char_filter(*ch) => editor.insert_char(terminal.frame(), *ch),
        key!(Paste str) => {
            // It seems that when pasting, '\n' are converted into '\r' for some
            // reason. I don't really know why, or at what point this happens.
            // Vim converts any '\r' pasted via the terminal into '\n', so I
            // decided to mirror that behaviour.
            let str = str.replace('\r', "\n");
            if str.chars().all(char_filter) {
                editor.insert_str(terminal.frame(), &str);
            } else {
                return false;
            }
        }
        key!(Ctrl + 'h') | key!(Backspace) => editor.backspace(terminal.frame()),
        key!(Ctrl + 'd') | key!(Delete) => editor.delete(),
        key!(Ctrl + 'l') => editor.clear(),
        // TODO Key bindings to delete words

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

pub fn list_editor_key_bindings_allowing_external_editing(
    bindings: &mut KeyBindingsList,
    char_filter: impl Fn(char) -> bool,
) {
    list_editor_editing_key_bindings(bindings, char_filter);
    bindings.binding("ctrl+x", "edit in external editor");
    bindings.empty();
    list_editor_cursor_movement_key_bindings(bindings);
}

pub fn handle_editor_input_event_allowing_external_editing(
    editor: &EditorState,
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    event: &InputEvent,
    char_filter: impl Fn(char) -> bool,
) -> io::Result<bool> {
    if let key!(Ctrl + 'x') = event {
        editor.edit_externally(terminal, crossterm_lock)?;
        Ok(true)
    } else {
        Ok(handle_editor_input_event(
            editor,
            terminal,
            event,
            char_filter,
        ))
    }
}
