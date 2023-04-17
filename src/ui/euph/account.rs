use crossterm::style::Stylize;
use euphoxide::api::PersonalAccountView;
use euphoxide::conn;
use toss::widgets::{BoxedAsync, EditorState, Empty, Join3, Join4, Text};
use toss::{Style, Terminal, WidgetExt};

use crate::euph::{self, Room};
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::widgets::Popup;
use crate::ui::{util, UiError};

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

    fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        let bold = Style::new().bold();
        Join4::vertical(
            Text::new(("Not logged in", bold.yellow())).segment(),
            Empty::new().with_height(1).segment(),
            Join3::horizontal(
                Text::new(("Email address:", bold))
                    .segment()
                    .with_fixed(true),
                Empty::new().with_width(1).segment().with_fixed(true),
                self.email
                    .widget()
                    .with_focus(self.focus == Focus::Email)
                    .segment(),
            )
            .segment(),
            Join3::horizontal(
                Text::new(("Password:", bold)).segment().with_fixed(true),
                Empty::new().with_width(5 + 1).segment().with_fixed(true),
                self.password
                    .widget()
                    .with_focus(self.focus == Focus::Password)
                    .with_hidden_default_placeholder()
                    .segment(),
            )
            .segment(),
        )
        .boxed_async()
    }
}

pub struct LoggedIn(PersonalAccountView);

impl LoggedIn {
    fn widget(&self) -> BoxedAsync<'_, UiError> {
        let bold = Style::new().bold();
        Join3::vertical(
            Text::new(("Logged in", bold.green())).segment(),
            Empty::new().with_height(1).segment(),
            Join3::horizontal(
                Text::new(("Email address:", bold))
                    .segment()
                    .with_fixed(true),
                Empty::new().with_width(1).segment().with_fixed(true),
                Text::new((&self.0.email,)).segment(),
            )
            .segment(),
        )
        .boxed_async()
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
    pub fn stabilize(&mut self, state: Option<&euph::State>) -> bool {
        if let Some(euph::State::Connected(_, conn::State::Joined(state))) = state {
            match (&self, &state.account) {
                (Self::LoggedOut(_), Some(view)) => *self = Self::LoggedIn(LoggedIn(view.clone())),
                (Self::LoggedIn(_), None) => *self = Self::LoggedOut(LoggedOut::new()),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    pub fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        let inner = match self {
            Self::LoggedOut(logged_out) => logged_out.widget(),
            Self::LoggedIn(logged_in) => logged_in.widget(),
        }
        .resize()
        .with_min_width(40);

        Popup::new(inner, "Account").boxed_async()
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
                util::list_editor_key_bindings(bindings, |c| c != '\n');
            }
            Self::LoggedIn(_) => bindings.binding("L", "log out"),
        }
    }

    pub fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
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
                            &mut logged_out.email,
                            terminal,
                            event,
                            |c| c != '\n',
                        ) {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    Focus::Password => {
                        if let key!(Enter) = event {
                            if let Some(room) = room {
                                let _ = room.login(
                                    logged_out.email.text().to_string(),
                                    logged_out.password.text().to_string(),
                                );
                            }
                            return EventResult::Handled;
                        }

                        if util::handle_editor_input_event(
                            &mut logged_out.password,
                            terminal,
                            event,
                            |c| c != '\n',
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
