use cove_config::Keys;
use cove_input::InputEvent;
use crossterm::style::Stylize;
use euphoxide::api::PersonalAccountView;
use euphoxide::conn;
use toss::widgets::{EditorState, Empty, Join3, Join4, Join5, Text};
use toss::{Style, Widget, WidgetExt};

use crate::euph::{self, Room};
use crate::ui::widgets::Popup;
use crate::ui::{UiError, util};

use super::popup::PopupResult;

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

    fn widget(&mut self) -> impl Widget<UiError> + '_ {
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
    }
}

pub struct LoggedIn(PersonalAccountView);

impl LoggedIn {
    fn widget(&self) -> impl Widget<UiError> + use<> {
        let bold = Style::new().bold();
        Join5::vertical(
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
            Empty::new().with_height(1).segment(),
            Text::new(("Log out", Style::new().black().on_white())).segment(),
        )
    }
}

pub enum AccountUiState {
    LoggedOut(LoggedOut),
    LoggedIn(LoggedIn),
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

    pub fn widget(&mut self) -> impl Widget<UiError> + '_ {
        let inner = match self {
            Self::LoggedOut(logged_out) => logged_out.widget().first2(),
            Self::LoggedIn(logged_in) => logged_in.widget().second2(),
        }
        .resize()
        .with_min_width(40);

        Popup::new(inner, "Account")
    }

    pub fn handle_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        room: &Option<Room>,
    ) -> PopupResult {
        if event.matches(&keys.general.abort) {
            return PopupResult::Close;
        }

        match self {
            Self::LoggedOut(logged_out) => {
                if event.matches(&keys.general.focus) {
                    logged_out.focus = match logged_out.focus {
                        Focus::Email => Focus::Password,
                        Focus::Password => Focus::Email,
                    };
                    return PopupResult::Handled;
                }

                match logged_out.focus {
                    Focus::Email => {
                        if event.matches(&keys.general.confirm) {
                            logged_out.focus = Focus::Password;
                            return PopupResult::Handled;
                        }

                        if util::handle_editor_input_event(
                            &mut logged_out.email,
                            event,
                            keys,
                            |c| c != '\n',
                        ) {
                            return PopupResult::Handled;
                        }
                    }
                    Focus::Password => {
                        if event.matches(&keys.general.confirm) {
                            if let Some(room) = room {
                                let _ = room.login(
                                    logged_out.email.text().to_string(),
                                    logged_out.password.text().to_string(),
                                );
                            }
                            return PopupResult::Handled;
                        }

                        if util::handle_editor_input_event(
                            &mut logged_out.password,
                            event,
                            keys,
                            |c| c != '\n',
                        ) {
                            return PopupResult::Handled;
                        }
                    }
                }
            }
            Self::LoggedIn(_) => {
                if event.matches(&keys.general.confirm) {
                    if let Some(room) = room {
                        let _ = room.logout();
                    }
                    return PopupResult::Handled;
                }
            }
        }

        PopupResult::NotHandled
    }
}
