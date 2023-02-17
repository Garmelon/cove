use crossterm::style::Stylize;
use toss::{Style, Styled};

use crate::ui::widgets::float::Float;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

pub enum RoomPopup {
    Error { description: String, reason: String },
}

impl RoomPopup {
    fn server_error_widget(description: &str, reason: &str) -> BoxedWidget {
        let border_style = Style::new().red().bold();
        let text = Styled::new_plain(description)
            .then_plain("\n\n")
            .then("Reason:", Style::new().bold())
            .then_plain(" ")
            .then_plain(reason);
        Popup::new(Text::new(text))
            .title(("Error", border_style))
            .border(border_style)
            .build()
    }

    pub fn widget(&self) -> BoxedWidget {
        let widget = match self {
            Self::Error {
                description,
                reason,
            } => Self::server_error_widget(description, reason),
        };

        Float::new(widget).horizontal(0.5).vertical(0.5).into()
    }
}
