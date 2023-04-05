use async_trait::async_trait;
use toss::{Frame, Pos, Size, WidthDb};

use super::Widget;

pub struct HRule;

#[async_trait]
impl Widget for HRule {
    async fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Size {
        Size::new(0, 1)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        for x in 0..size.width as i32 {
            frame.write(Pos::new(x, 0), "─");
        }
    }
}

pub struct VRule;

#[async_trait]
impl Widget for VRule {
    async fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Size {
        Size::new(1, 0)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        for y in 0..size.height as i32 {
            frame.write(Pos::new(0, y), "│");
        }
    }
}
