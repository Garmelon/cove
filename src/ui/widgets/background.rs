use async_trait::async_trait;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use super::Widget;

pub struct Background {
    inner: Box<dyn Widget + Send>,
    style: ContentStyle,
}

impl Background {
    pub fn new<W: 'static + Widget + Send>(inner: W, style: ContentStyle) -> Self {
        Self {
            inner: Box::new(inner),
            style,
        }
    }
}

#[async_trait]
impl Widget for Background {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        self.inner.size(frame, max_width, max_height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame, pos: Pos, size: Size) {
        for dy in 0..size.height {
            for dx in 0..size.width {
                frame.write(pos + Pos::new(dx.into(), dy.into()), (" ", self.style));
            }
        }

        self.inner.render(frame, pos, size).await;
    }
}
