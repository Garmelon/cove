use crossterm::style::{ContentStyle, Stylize};
use toss::styled::Styled;

use crate::ui::widgets::background::Background;
use crate::ui::widgets::border::Border;
use crate::ui::widgets::layer::Layer;
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

pub enum Popup {
    ServerError { description: String, reason: String },
}

impl Popup {
    fn server_error_widget(description: &str, reason: &str) -> BoxedWidget {
        let border_style = ContentStyle::default().red().bold();
        let text = Styled::new_plain(description)
            .then_plain("\n\n")
            .then_plain(reason);
        Layer::new(vec![
            Padding::new(Text::new(("Error", border_style)))
                .horizontal(1)
                .into(),
            Border::new(Background::new(Text::new(text)))
                .style(border_style)
                .into(),
        ])
        .into()
    }

    pub fn widget(&self) -> BoxedWidget {
        match self {
            Self::ServerError {
                description,
                reason,
            } => Self::server_error_widget(description, reason),
        }
    }
}
