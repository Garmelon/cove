use std::io;
use std::sync::Arc;

use parking_lot::FairMutex;
use toss::widgets::EditorState;
use toss::Terminal;

use super::input::{key, InputEvent, KeyBindingsList};
use super::widgets2::ListState;

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

//////////
// List //
//////////

pub fn list_list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("j/k, ↓/↑", "move cursor up/down");
    bindings.binding("g, home", "move cursor to top");
    bindings.binding("G, end", "move cursor to bottom");
    bindings.binding("ctrl+y/e", "scroll up/down");
}

pub fn handle_list_input_event<Id: Clone>(list: &mut ListState<Id>, event: &InputEvent) -> bool {
    match event {
        key!('k') | key!(Up) => list.move_cursor_up(),
        key!('j') | key!(Down) => list.move_cursor_down(),
        key!('g') | key!(Home) => list.move_cursor_to_top(),
        key!('G') | key!(End) => list.move_cursor_to_bottom(),
        key!(Ctrl + 'y') => list.scroll_up(1),
        key!(Ctrl + 'e') => list.scroll_down(1),
        _ => return false,
    }

    true
}

////////////
// Editor //
////////////

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
    editor: &mut EditorState,
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
        }) if char_filter('\n') => editor.insert_char(terminal.widthdb(), '\n'),

        // Editing
        key!(Char ch) if char_filter(*ch) => editor.insert_char(terminal.widthdb(), *ch),
        key!(Paste str) => {
            // It seems that when pasting, '\n' are converted into '\r' for some
            // reason. I don't really know why, or at what point this happens.
            // Vim converts any '\r' pasted via the terminal into '\n', so I
            // decided to mirror that behaviour.
            let str = str.replace('\r', "\n");
            if str.chars().all(char_filter) {
                editor.insert_str(terminal.widthdb(), &str);
            } else {
                return false;
            }
        }
        key!(Ctrl + 'h') | key!(Backspace) => editor.backspace(terminal.widthdb()),
        key!(Ctrl + 'd') | key!(Delete) => editor.delete(),
        key!(Ctrl + 'l') => editor.clear(),
        // TODO Key bindings to delete words

        // Cursor movement
        key!(Ctrl + 'b') | key!(Left) => editor.move_cursor_left(terminal.widthdb()),
        key!(Ctrl + 'f') | key!(Right) => editor.move_cursor_right(terminal.widthdb()),
        key!(Alt + 'b') | key!(Ctrl + Left) => editor.move_cursor_left_a_word(terminal.widthdb()),
        key!(Alt + 'f') | key!(Ctrl + Right) => editor.move_cursor_right_a_word(terminal.widthdb()),
        key!(Ctrl + 'a') | key!(Home) => editor.move_cursor_to_start_of_line(terminal.widthdb()),
        key!(Ctrl + 'e') | key!(End) => editor.move_cursor_to_end_of_line(terminal.widthdb()),
        key!(Up) => editor.move_cursor_up(terminal.widthdb()),
        key!(Down) => editor.move_cursor_down(terminal.widthdb()),

        _ => return false,
    }

    true
}

fn edit_externally(
    editor: &mut EditorState,
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
) -> io::Result<()> {
    let text = prompt(terminal, crossterm_lock, editor.text())?;

    if text.trim().is_empty() {
        // The user likely wanted to abort the edit and has deleted the
        // entire text (bar whitespace left over by some editors).
        return Ok(());
    }

    if let Some(text) = text.strip_suffix('\n') {
        // Some editors like vim add a trailing newline that would look out of
        // place in cove's editors. To intentionally add a trailing newline,
        // simply add two in-editor.
        editor.set_text(terminal.widthdb(), text.to_string());
    } else {
        editor.set_text(terminal.widthdb(), text);
    }

    Ok(())
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
    editor: &mut EditorState,
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    event: &InputEvent,
    char_filter: impl Fn(char) -> bool,
) -> io::Result<bool> {
    if let key!(Ctrl + 'x') = event {
        edit_externally(editor, terminal, crossterm_lock)?;
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
