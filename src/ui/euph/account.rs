use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use euphoxide::api::PersonalAccountView;
use euphoxide::conn::Status;
use parking_lot::FairMutex;
use toss::terminal::Terminal;

use crate::euph::Room;
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::util;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::join::{HJoin, Segment, VJoin};
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::resize::Resize;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

use super::room::RoomStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Email,
    Password,
}

pub struct LoggedOut {
    focus: Focus,
    email: EditorState,
    password: EditorState,
}

impl LoggedOut {
    fn new() -> Self {
        Self {
            focus: Focus::Email,
            email: EditorState::new(),
            password: EditorState::new(),
        }
    }

    fn widget(&self) -> BoxedWidget {
        let bold = ContentStyle::default().bold();
        VJoin::new(vec![
            Segment::new(Text::new(("Not logged in", bold.yellow()))),
            Segment::new(Empty::new().height(1)),
            Segment::new(HJoin::new(vec![
                Segment::new(Text::new(("Email address:", bold))),
                Segment::new(Empty::new().width(1)),
                Segment::new(self.email.widget().focus(self.focus == Focus::Email)),
            ])),
            Segment::new(HJoin::new(vec![
                Segment::new(Text::new(("Password:", bold))),
                Segment::new(Empty::new().width(5 + 1)),
                Segment::new(
                    self.password
                        .widget()
                        .focus(self.focus == Focus::Password)
                        .hidden(),
                ),
            ])),
        ])
        .into()
    }
}

pub struct LoggedIn(PersonalAccountView);

impl LoggedIn {
    fn widget(&self) -> BoxedWidget {
        let bold = ContentStyle::default().bold();
        VJoin::new(vec![
            Segment::new(Text::new(("Logged in", bold.green()))),
            Segment::new(Empty::new().height(1)),
            Segment::new(HJoin::new(vec![
                Segment::new(Text::new(("Email address:", bold))),
                Segment::new(Empty::new().width(1)),
                Segment::new(Text::new((&self.0.email,))),
            ])),
        ])
        .into()
    }
}

pub enum AccountUiState {
    LoggedOut(LoggedOut),
    LoggedIn(LoggedIn),
}

pub enum EventResult {
    NotHandled,
    Handled,
    ResetState,
}

impl AccountUiState {
    pub fn new() -> Self {
        Self::LoggedOut(LoggedOut::new())
    }

    /// Returns `false` if the account UI should not be displayed any longer.
    pub fn stabilize(&mut self, status: &RoomStatus) -> bool {
        if let RoomStatus::Connected(Status::Joined(status)) = status {
            match (&self, &status.account) {
                (Self::LoggedOut(_), Some(view)) => *self = Self::LoggedIn(LoggedIn(view.clone())),
                (Self::LoggedIn(_), None) => *self = Self::LoggedOut(LoggedOut::new()),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    pub fn widget(&self) -> BoxedWidget {
        let inner = match self {
            Self::LoggedOut(logged_out) => logged_out.widget(),
            Self::LoggedIn(logged_in) => logged_in.widget(),
        };
        Popup::new(Resize::new(inner).min_width(40))
            .title("Account")
            .build()
    }

    pub fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close account ui");

        match self {
            Self::LoggedOut(logged_out) => {
                match logged_out.focus {
                    Focus::Email => bindings.binding("enter", "focus on password"),
                    Focus::Password => bindings.binding("enter", "log in"),
                }
                bindings.binding("tab", "switch focus");
                util::list_editor_key_bindings(bindings, |c| c != '\n', false);
            }
            Self::LoggedIn(_) => bindings.binding("L", "log out"),
        }
    }

    pub fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        room: &Option<Room>,
    ) -> EventResult {
        if let key!(Esc) = event {
            return EventResult::ResetState;
        }

        match self {
            Self::LoggedOut(logged_out) => {
                if let key!(Tab) = event {
                    logged_out.focus = match logged_out.focus {
                        Focus::Email => Focus::Password,
                        Focus::Password => Focus::Email,
                    };
                    return EventResult::Handled;
                }

                match logged_out.focus {
                    Focus::Email => {
                        if let key!(Enter) = event {
                            logged_out.focus = Focus::Password;
                            return EventResult::Handled;
                        }

                        if util::handle_editor_input_event(
                            &logged_out.email,
                            terminal,
                            crossterm_lock,
                            event,
                            |c| c != '\n',
                            false,
                        ) {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    Focus::Password => {
                        if let key!(Enter) = event {
                            if let Some(room) = room {
                                let _ =
                                    room.login(logged_out.email.text(), logged_out.password.text());
                            }
                            return EventResult::Handled;
                        }

                        if util::handle_editor_input_event(
                            &logged_out.password,
                            terminal,
                            crossterm_lock,
                            event,
                            |c| c != '\n',
                            false,
                        ) {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                }
            }
            Self::LoggedIn(_) => {
                if let key!('L') = event {
                    if let Some(room) = room {
                        let _ = room.logout();
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }
}
