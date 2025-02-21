use std::io;

use crossterm::style::Stylize;
use toss::widgets::Text;
use toss::{Style, Styled, Widget};

use crate::ui::widgets::Popup;
use crate::ui::UiError;

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
    ErrorOpeningLink { link: String, error: io::Error },
}
