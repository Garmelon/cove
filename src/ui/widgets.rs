pub mod background;
pub mod empty;
pub mod join;
pub mod list;
pub mod rules;
pub mod text;

use async_trait::async_trait;
use toss::frame::{Frame, Size};

#[async_trait]
pub trait Widget {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size;

    async fn render(self: Box<Self>, frame: &mut Frame);
}
