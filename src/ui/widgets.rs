pub mod list;
pub mod text;

use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

#[async_trait]
pub trait Widget {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size;

    async fn render(self, frame: &mut Frame, pos: Pos, size: Size);
}
