use async_trait::async_trait;
use crossterm::style::{ContentStyle, Stylize};
use toss::frame::{Frame, Pos, Size};

use crate::ui::widgets::Widget;

pub const INDENT: &str = "│ ";
pub const INDENT_WIDTH: usize = 2;

pub fn style() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

pub fn style_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub struct Indent {
    level: usize,
    highlighted: bool,
}

impl Indent {
    pub fn new(level: usize, highlighted: bool) -> Self {
        Self { level, highlighted }
    }
}

#[async_trait]
impl Widget for Indent {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        Size::new((INDENT_WIDTH * self.level) as u16, 0)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        let style = if self.highlighted {
            style_inverted()
        } else {
            style()
        };

        for y in 0..size.height {
            frame.write(Pos::new(0, y.into()), (INDENT.repeat(self.level), style))
        }
    }
}
