// Since the widget module is effectively a library and will probably be moved
// to toss later, warnings about unused functions are mostly inaccurate.
#![allow(dead_code)]

pub mod background;
pub mod border;
pub mod empty;
pub mod float;
pub mod join;
pub mod layer;
pub mod list;
pub mod padding;
pub mod rules;
pub mod text;

use async_trait::async_trait;
use toss::frame::{Frame, Size};

#[async_trait]
pub trait Widget {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size;

    async fn render(self: Box<Self>, frame: &mut Frame);
}

pub type BoxedWidget = Box<dyn Widget + Send>;

impl<W: 'static + Widget + Send> From<W> for BoxedWidget {
    fn from(widget: W) -> Self {
        Box::new(widget)
    }
}
