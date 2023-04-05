use async_trait::async_trait;
use toss::{Frame, Pos, Size, Style, WidthDb};

use crate::ui::widgets::Widget;

pub const INDENT: &str = "│ ";
pub const INDENT_WIDTH: usize = 2;

pub struct Indent {
    level: usize,
    style: Style,
}

impl Indent {
    pub fn new(level: usize, style: Style) -> Self {
        Self { level, style }
    }
}

#[async_trait]
impl Widget for Indent {
    async fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Size {
        Size::new((INDENT_WIDTH * self.level) as u16, 0)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        for y in 0..size.height {
            frame.write(
                Pos::new(0, y.into()),
                (INDENT.repeat(self.level), self.style),
            )
        }
    }
}
