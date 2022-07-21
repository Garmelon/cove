use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

pub struct Border(BoxedWidget);

impl Border {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self(inner.into())
    }
}

#[async_trait]
impl Widget for Border {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let max_width = max_width.map(|w| w.saturating_sub(2));
        let max_height = max_height.map(|h| h.saturating_sub(2));
        let size = self.0.size(frame, max_width, max_height);
        size + Size::new(2, 2)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let mut size = frame.size();
        size.width = size.width.max(2);
        size.height = size.height.max(2);

        let right = size.width as i32 - 1;
        let bottom = size.height as i32 - 1;
        frame.write(Pos::new(0, 0), "┌");
        frame.write(Pos::new(right, 0), "┐");
        frame.write(Pos::new(0, bottom), "└");
        frame.write(Pos::new(right, bottom), "┘");

        for y in 1..bottom {
            frame.write(Pos::new(0, y), "│");
            frame.write(Pos::new(right, y), "│");
        }

        for x in 1..right {
            frame.write(Pos::new(x, 0), "─");
            frame.write(Pos::new(x, bottom), "─");
        }

        frame.push(Pos::new(1, 1), size - Size::new(2, 2));
        self.0.render(frame).await;
        frame.pop();
    }
}
