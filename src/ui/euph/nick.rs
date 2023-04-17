use euphoxide::conn::Joined;
use toss::widgets::{BoxedAsync, EditorState};
use toss::{Style, Terminal, WidgetExt};

use crate::euph::{self, Room};
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::widgets::Popup;
use crate::ui::{util, UiError};

pub fn new(joined: Joined) -> EditorState {
    EditorState::with_initial_text(joined.session.name)
}

pub fn widget(editor: &mut EditorState) -> BoxedAsync<'_, UiError> {
    let inner = editor
        .widget()
        .with_highlight(|s| euph::style_nick_exact(s, Style::new()));

    Popup::new(inner, "Choose nick").boxed_async()
}

fn nick_char(c: char) -> bool {
    c != '\n'
}

pub fn list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("esc", "abort");
    bindings.binding("enter", "set nick");
    util::list_editor_key_bindings(bindings, nick_char);
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
                let _ = room.nick(editor.text().to_string());
            }
            EventResult::ResetState
        }
        _ => {
            if util::handle_editor_input_event(editor, terminal, event, nick_char) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        }
    }
}
