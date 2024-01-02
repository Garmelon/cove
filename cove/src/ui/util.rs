use cove_config::Keys;
use cove_input::InputEvent;
use crossterm::event::{KeyCode, KeyModifiers};
use toss::widgets::EditorState;

use super::widgets::ListState;

/// Test if a character is allowed to be typed in a room name.
pub fn is_room_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

//////////
// List //
//////////

pub fn handle_list_input_event<Id: Clone>(
    list: &mut ListState<Id>,
    event: &InputEvent<'_>,
    keys: &Keys,
) -> bool {
    // Cursor movement
    if event.matches(&keys.cursor.up) {
        list.move_cursor_up();
        return true;
    }
    if event.matches(&keys.cursor.down) {
        list.move_cursor_down();
        return true;
    }
    if event.matches(&keys.cursor.to_top) {
        list.move_cursor_to_top();
        return true;
    }
    if event.matches(&keys.cursor.to_bottom) {
        list.move_cursor_to_bottom();
        return true;
    }

    // Scrolling
    if event.matches(&keys.scroll.up_line) {
        list.scroll_up(1);
        return true;
    }
    if event.matches(&keys.scroll.down_line) {
        list.scroll_down(1);
        return true;
    }
    if event.matches(&keys.scroll.up_half) {
        list.scroll_up_half();
        return true;
    }
    if event.matches(&keys.scroll.down_half) {
        list.scroll_down_half();
        return true;
    }
    if event.matches(&keys.scroll.up_full) {
        list.scroll_up_full();
        return true;
    }
    if event.matches(&keys.scroll.down_full) {
        list.scroll_down_full();
        return true;
    }
    if event.matches(&keys.scroll.center_cursor) {
        list.center_cursor();
        return true;
    }

    false
}

////////////
// Editor //
////////////

fn edit_externally(
    editor: &mut EditorState,
    event: &mut InputEvent<'_>,
    char_filter: impl Fn(char) -> bool,
) {
    let Ok(text) = event.prompt(editor.text()) else {
        // Something went wrong during editing, let's abort the edit.
        return;
    };

    if text.trim().is_empty() {
        // The user likely wanted to abort the edit and has deleted the
        // entire text (bar whitespace left over by some editors).
        return;
    }

    let text = text
        .strip_suffix('\n')
        .unwrap_or(&text)
        .chars()
        .filter(|c| char_filter(*c))
        .collect::<String>();

    editor.set_text(event.widthdb(), text);
}

fn char_modifier(modifiers: KeyModifiers) -> bool {
    modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
}

pub fn handle_editor_input_event(
    editor: &mut EditorState,
    event: &mut InputEvent<'_>,
    keys: &Keys,
    char_filter: impl Fn(char) -> bool,
) -> bool {
    // Cursor movement
    if event.matches(&keys.editor.cursor.left) {
        editor.move_cursor_left(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.right) {
        editor.move_cursor_right(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.left_word) {
        editor.move_cursor_left_a_word(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.right_word) {
        editor.move_cursor_right_a_word(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.start) {
        editor.move_cursor_to_start_of_line(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.end) {
        editor.move_cursor_to_end_of_line(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.up) {
        editor.move_cursor_up(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.cursor.down) {
        editor.move_cursor_down(event.widthdb());
        return true;
    }

    // Editing
    if event.matches(&keys.editor.action.backspace) {
        editor.backspace(event.widthdb());
        return true;
    }
    if event.matches(&keys.editor.action.delete) {
        editor.delete();
        return true;
    }
    if event.matches(&keys.editor.action.clear) {
        editor.clear();
        return true;
    }
    if event.matches(&keys.editor.action.external) {
        edit_externally(editor, event, char_filter);
        return true;
    }

    // Inserting individual characters
    if let Some(key_event) = event.key_event() {
        match key_event.code {
            KeyCode::Enter if char_filter('\n') => {
                editor.insert_char(event.widthdb(), '\n');
                return true;
            }
            KeyCode::Char(c) if char_modifier(key_event.modifiers) && char_filter(c) => {
                editor.insert_char(event.widthdb(), c);
                return true;
            }
            _ => {}
        }
    }

    // Pasting text
    if let Some(text) = event.paste_event() {
        // It seems that when pasting, '\n' are converted into '\r' for some
        // reason. I don't really know why, or at what point this happens. Vim
        // converts any '\r' pasted via the terminal into '\n', so I decided to
        // mirror that behaviour.
        let text = text
            .chars()
            .map(|c| if c == '\r' { '\n' } else { c })
            .filter(|c| char_filter(*c))
            .collect::<String>();
        editor.insert_str(event.widthdb(), &text);
        return true;
    }

    false
}
