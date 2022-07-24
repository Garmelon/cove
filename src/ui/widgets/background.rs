use async_trait::async_trait;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

pub struct Background {
    inner: BoxedWidget,
    style: ContentStyle,
}

impl Background {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            style: ContentStyle::default(),
        }
    }

    pub fn style(mut self, style: ContentStyle) -> Self {
        self.style = style;
        self
    }
}

#[async_trait]
impl Widget for Background {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        self.inner.size(frame, max_width, max_height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        for dy in 0..size.height {
            for dx in 0..size.width {
                frame.write(Pos::new(dx.into(), dy.into()), (" ", self.style));
            }
        }

        self.inner.render(frame).await;
    }
}
