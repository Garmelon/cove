use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use euphoxide::conn::{Joining, Status};
use parking_lot::FairMutex;
use toss::terminal::Terminal;

use crate::euph::Room;
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::util;
use crate::ui::widgets::cursor::Cursor;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

use super::room::RoomStatus;

pub fn new() -> EditorState {
    EditorState::new()
}

pub fn stable(status: &RoomStatus) -> bool {
    matches!(
        status,
        RoomStatus::Connected(Status::Joining(Joining {
            bounce: Some(_),
            ..
        }))
    )
}

pub fn widget() -> BoxedWidget {
    Popup::new(
        Padding::new(Cursor::new(Text::new((
            "<hidden>",
            ContentStyle::default().grey().italic(),
        ))))
        .left(1),
    )
    .title("Enter password")
    .inner_padding(false)
    .build()
}

pub fn list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("esc", "abort");
    bindings.binding("enter", "authenticate");
    util::list_editor_key_bindings(bindings, |_| true, false);
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
                let _ = room.auth(editor.text());
            }
            EventResult::ResetState
        }
        _ => {
            if util::handle_editor_input_event(
                editor,
                terminal,
                crossterm_lock,
                event,
                |_| true,
                false,
            ) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        }
    }
}
