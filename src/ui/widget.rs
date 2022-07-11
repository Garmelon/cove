use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

#[async_trait]
pub trait Widget {
    fn size(max_width: Option<u16>, max_height: Option<u16>) -> Size;

    async fn render(frame: &mut Frame, pos: Pos, size: Size);
}
