use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

pub struct Cursor {
    inner: BoxedWidget,
    pos: Pos,
}

impl Cursor {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            pos: Pos::ZERO,
        }
    }

    pub fn at(mut self, pos: Pos) -> Self {
        self.pos = pos;
        self
    }

    pub fn at_xy(self, x: i32, y: i32) -> Self {
        self.at(Pos::new(x, y))
    }
}

#[async_trait]
impl Widget for Cursor {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        self.inner.size(frame, max_width, max_height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        self.inner.render(frame).await;
        frame.set_cursor(Some(self.pos));
    }
}
