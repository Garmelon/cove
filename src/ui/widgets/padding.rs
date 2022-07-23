use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

pub struct Padding {
    inner: BoxedWidget,
    left: u16,
    right: u16,
    top: u16,
    bottom: u16,
}

impl Padding {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        }
    }

    pub fn left(mut self, amount: u16) -> Self {
        self.left = amount;
        self
    }

    pub fn right(mut self, amount: u16) -> Self {
        self.right = amount;
        self
    }

    pub fn horizontal(self, amount: u16) -> Self {
        self.left(amount).right(amount)
    }

    pub fn top(mut self, amount: u16) -> Self {
        self.top = amount;
        self
    }

    pub fn bottom(mut self, amount: u16) -> Self {
        self.bottom = amount;
        self
    }

    pub fn vertical(self, amount: u16) -> Self {
        self.top(amount).bottom(amount)
    }

    pub fn all(self, amount: u16) -> Self {
        self.horizontal(amount).vertical(amount)
    }
}

#[async_trait]
impl Widget for Padding {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let horizontal = self.left + self.right;
        let vertical = self.top + self.bottom;

        let max_width = max_width.map(|w| w.saturating_sub(horizontal));
        let max_height = max_height.map(|h| h.saturating_sub(vertical));

        let size = self.inner.size(frame, max_width, max_height);

        size + Size::new(horizontal, vertical)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        let inner_pos = Pos::new(self.left.into(), self.top.into());
        let inner_size = Size::new(
            size.width.saturating_sub(self.left + self.right),
            size.height.saturating_sub(self.top + self.bottom),
        );

        frame.push(inner_pos, inner_size);
        self.inner.render(frame).await;
        frame.pop();
    }
}
