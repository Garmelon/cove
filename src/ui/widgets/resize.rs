use async_trait::async_trait;
use toss::{Frame, Size};

use super::{BoxedWidget, Widget};

pub struct Resize {
    inner: BoxedWidget,
    min_width: Option<u16>,
    min_height: Option<u16>,
    max_width: Option<u16>,
    max_height: Option<u16>,
}

impl Resize {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
        }
    }

    pub fn min_width(mut self, amount: u16) -> Self {
        self.min_width = Some(amount);
        self
    }

    pub fn max_width(mut self, amount: u16) -> Self {
        self.max_width = Some(amount);
        self
    }

    pub fn min_height(mut self, amount: u16) -> Self {
        self.min_height = Some(amount);
        self
    }

    pub fn max_height(mut self, amount: u16) -> Self {
        self.max_height = Some(amount);
        self
    }
}

#[async_trait]
impl Widget for Resize {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let max_width = match (max_width, self.max_width) {
            (None, None) => None,
            (Some(w), None) => Some(w),
            (None, Some(sw)) => Some(sw),
            (Some(w), Some(sw)) => Some(w.min(sw)),
        };

        let max_height = match (max_height, self.max_height) {
            (None, None) => None,
            (Some(h), None) => Some(h),
            (None, Some(sh)) => Some(sh),
            (Some(h), Some(sh)) => Some(h.min(sh)),
        };

        let size = self.inner.size(frame, max_width, max_height);

        let width = match self.min_width {
            Some(min_width) => size.width.max(min_width),
            None => size.width,
        };

        let height = match self.min_height {
            Some(min_height) => size.height.max(min_height),
            None => size.height,
        };

        Size::new(width, height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        self.inner.render(frame).await;
    }
}
