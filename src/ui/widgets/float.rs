use async_trait::async_trait;
use toss::{Frame, Pos, Size, WidthDb};

use super::{BoxedWidget, Widget};

pub struct Float {
    inner: BoxedWidget,
    horizontal: Option<f32>,
    vertical: Option<f32>,
}

impl Float {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
            horizontal: None,
            vertical: None,
        }
    }

    pub fn horizontal(mut self, position: f32) -> Self {
        self.horizontal = Some(position);
        self
    }

    pub fn vertical(mut self, position: f32) -> Self {
        self.vertical = Some(position);
        self
    }
}

#[async_trait]
impl Widget for Float {
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Size {
        self.inner.size(widthdb, max_width, max_height).await
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        let mut inner_size = self
            .inner
            .size(frame.widthdb(), Some(size.width), Some(size.height))
            .await;
        inner_size.width = inner_size.width.min(size.width);
        inner_size.height = inner_size.height.min(size.height);

        let mut inner_pos = Pos::ZERO;

        if let Some(horizontal) = self.horizontal {
            let available = (size.width - inner_size.width) as f32;
            // Biased towards the left if horizontal lands exactly on the
            // boundary between two cells
            inner_pos.x = (horizontal * available).floor().min(available) as i32;
        }

        if let Some(vertical) = self.vertical {
            let available = (size.height - inner_size.height) as f32;
            // Biased towards the top if vertical lands exactly on the boundary
            // between two cells
            inner_pos.y = (vertical * available).floor().min(available) as i32;
        }

        frame.push(inner_pos, inner_size);
        self.inner.render(frame).await;
        frame.pop();
    }
}
