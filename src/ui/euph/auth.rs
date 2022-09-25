use toss::terminal::Terminal;

use crate::euph::Room;
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::util;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::BoxedWidget;

pub fn new() -> EditorState {
    EditorState::new()
}

pub fn widget(editor: &EditorState) -> BoxedWidget {
    Popup::new(editor.widget().hidden())
        .title("Enter password")
        .build()
}

pub fn list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("esc", "abort");
    bindings.binding("enter", "authenticate");
    util::list_editor_key_bindings(bindings, |_| true);
}

pub enum EventResult {
    NotHandled,
    Handled,
    ResetState,
}

pub fn handle_input_event(
    terminal: &mut Terminal,
    event: &InputEvent,
    room: &Option<Room>,
    editor: &EditorState,
) -> EventResult {
    match event {
        key!(Esc) => EventResult::ResetState,
        key!(Enter) => {
            if let Some(room) = &room {
                let _ = room.auth(editor.text());
            }
            EventResult::ResetState
        }
        _ => {
            if util::handle_editor_input_event(editor, terminal, event, |_| true) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        }
    }
}
