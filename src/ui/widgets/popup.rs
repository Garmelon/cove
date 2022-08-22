use crossterm::style::ContentStyle;
use toss::styled::Styled;

use super::background::Background;
use super::border::Border;
use super::float::Float;
use super::layer::Layer;
use super::padding::Padding;
use super::text::Text;
use super::BoxedWidget;

pub struct Popup {
    inner: BoxedWidget,
    inner_padding: bool,
    title: Option<Styled>,
    border_style: ContentStyle,
    bg_style: ContentStyle,
}

impl Popup {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            inner_padding: true,
            title: None,
            border_style: ContentStyle::default(),
            bg_style: ContentStyle::default(),
        }
    }

    pub fn inner_padding(mut self, active: bool) -> Self {
        self.inner_padding = active;
        self
    }

    pub fn title<S: Into<Styled>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn border(mut self, style: ContentStyle) -> Self {
        self.border_style = style;
        self
    }

    pub fn background(mut self, style: ContentStyle) -> Self {
        self.bg_style = style;
        self
    }

    pub fn build(self) -> BoxedWidget {
        let inner = if self.inner_padding {
            Padding::new(self.inner).horizontal(1).into()
        } else {
            self.inner
        };
        let window =
            Border::new(Background::new(inner).style(self.bg_style)).style(self.border_style);

        let widget: BoxedWidget = if let Some(title) = self.title {
            let title = Float::new(
                Padding::new(
                    Background::new(Padding::new(Text::new(title)).horizontal(1))
                        .style(self.border_style),
                )
                .horizontal(2),
            );
            Layer::new(vec![window.into(), title.into()]).into()
        } else {
            window.into()
        };

        Float::new(widget).vertical(0.5).horizontal(0.5).into()
    }
}
