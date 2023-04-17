use toss::widgets::{BoxedAsync, EditorState};
use toss::{Terminal, WidgetExt};

use crate::euph::Room;
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::widgets::Popup;
use crate::ui::{util, UiError};

pub fn new() -> EditorState {
    EditorState::new()
}

pub fn widget(editor: &mut EditorState) -> BoxedAsync<'_, UiError> {
    Popup::new(
        editor.widget().with_hidden_default_placeholder(),
        "Enter password",
    )
    .boxed_async()
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
    editor: &mut EditorState,
) -> EventResult {
    match event {
        key!(Esc) => EventResult::ResetState,
        key!(Enter) => {
            if let Some(room) = &room {
                let _ = room.auth(editor.text().to_string());
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
