use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::Widget;

pub struct Empty;

#[async_trait]
impl Widget for Empty {
    fn size(&self, _frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        Size::ZERO
    }

    async fn render(self: Box<Self>, _frame: &mut Frame, _pos: Pos, _size: Size) {}
}
