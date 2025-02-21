use cove_config::Keys;
use cove_input::InputEvent;
use toss::{Widget, widgets::EditorState};

use crate::{
    euph::Room,
    ui::{UiError, util, widgets::Popup},
};

use super::popup::PopupResult;

pub fn new() -> EditorState {
    EditorState::new()
}

pub fn widget(editor: &mut EditorState) -> impl Widget<UiError> {
    Popup::new(
        editor.widget().with_hidden_default_placeholder(),
        "Enter password",
    )
}

pub fn handle_input_event(
    event: &mut InputEvent<'_>,
    keys: &Keys,
    room: &Option<Room>,
    editor: &mut EditorState,
) -> PopupResult {
    if event.matches(&keys.general.abort) {
        return PopupResult::Close;
    }

    if event.matches(&keys.general.confirm) {
        if let Some(room) = &room {
            let _ = room.auth(editor.text().to_string());
        }
        return PopupResult::Close;
    }

    if util::handle_editor_input_event(editor, event, keys, |_| true) {
        return PopupResult::Handled;
    }

    PopupResult::NotHandled
}
