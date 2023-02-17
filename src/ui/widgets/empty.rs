use async_trait::async_trait;
use toss::{Frame, Size};

use super::Widget;

#[derive(Debug, Default, Clone, Copy)]
pub struct Empty {
    size: Size,
}

impl Empty {
    pub fn new() -> Self {
        Self { size: Size::ZERO }
    }

    pub fn width(mut self, width: u16) -> Self {
        self.size.width = width;
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.size.height = height;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

#[async_trait]
impl Widget for Empty {
    fn size(&self, _frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        self.size
    }

    async fn render(self: Box<Self>, _frame: &mut Frame) {}
}
