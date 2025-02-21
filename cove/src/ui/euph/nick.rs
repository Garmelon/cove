use cove_config::Keys;
use cove_input::InputEvent;
use euphoxide::conn::Joined;
use toss::{Style, Widget, widgets::EditorState};

use crate::{
    euph::{self, Room},
    ui::{UiError, util, widgets::Popup},
};

use super::popup::PopupResult;

pub fn new(joined: Joined) -> EditorState {
    EditorState::with_initial_text(joined.session.name)
}

pub fn widget(editor: &mut EditorState) -> impl Widget<UiError> + '_ {
    let inner = editor
        .widget()
        .with_highlight(|s| euph::style_nick_exact(s, Style::new()));

    Popup::new(inner, "Choose nick")
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
            let _ = room.nick(editor.text().to_string());
        }
        return PopupResult::Close;
    }

    if util::handle_editor_input_event(editor, event, keys, |c| c != '\n') {
        return PopupResult::Handled;
    }

    PopupResult::NotHandled
}
