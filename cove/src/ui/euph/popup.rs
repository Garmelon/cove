use std::io;

use crossterm::style::Stylize;
use toss::{Style, Styled, Widget, widgets::Text};

use crate::ui::{UiError, widgets::Popup};

pub enum RoomPopup {
    Error { description: String, reason: String },
}

impl RoomPopup {
    fn server_error_widget(description: &str, reason: &str) -> impl Widget<UiError> + use<> {
        let border_style = Style::new().red().bold();
        let text = Styled::new_plain(description)
            .then_plain("\n\n")
            .then("Reason:", Style::new().bold())
            .then_plain(" ")
            .then_plain(reason);

        Popup::new(Text::new(text), ("Error", border_style)).with_border_style(border_style)
    }

    pub fn widget(&self) -> impl Widget<UiError> + use<> {
        match self {
            Self::Error {
                description,
                reason,
            } => Self::server_error_widget(description, reason),
        }
    }
}

pub enum PopupResult {
    NotHandled,
    Handled,
    Close,
    SwitchToRoom { name: String },
    ErrorOpeningLink { link: String, error: io::Error },
}
