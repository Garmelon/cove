use std::sync::Arc;

use euphoxide::conn::Joined;
use parking_lot::FairMutex;
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::{self, Room};
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::util;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::BoxedWidget;

pub fn new(joined: Joined) -> EditorState {
    EditorState::with_initial_text(joined.session.name)
}

pub fn widget(editor: &EditorState) -> BoxedWidget {
    let editor = editor
        .widget()
        .highlight(|s| Styled::new(s, euph::nick_style(s)));
    Popup::new(Padding::new(editor).left(1))
        .title("Choose nick")
        .inner_padding(false)
        .build()
}

fn nick_char(c: char) -> bool {
    c != '\n'
}

pub fn list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("esc", "abort");
    bindings.binding("enter", "set nick");
    util::list_editor_key_bindings(bindings, nick_char, false);
}

pub enum EventResult {
    NotHandled,
    Handled,
    ResetState,
}

pub fn handle_input_event(
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    event: &InputEvent,
    room: &Option<Room>,
    editor: &EditorState,
) -> EventResult {
    match event {
        key!(Esc) => EventResult::ResetState,
        key!(Enter) => {
            if let Some(room) = &room {
                let _ = room.nick(editor.text());
            }
            EventResult::ResetState
        }
        _ => {
            if util::handle_editor_input_event(
                editor,
                terminal,
                crossterm_lock,
                event,
                nick_char,
                false,
            ) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        }
    }
}
