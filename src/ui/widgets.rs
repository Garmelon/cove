// Since the widget module is effectively a library and will probably be moved
// to toss later, warnings about unused functions are mostly inaccurate.
// TODO Restrict this a bit more?
#![allow(dead_code)]

pub mod background;
pub mod border;
pub mod cursor;
pub mod editor;
pub mod empty;
pub mod float;
pub mod join;
pub mod layer;
pub mod list;
pub mod padding;
pub mod popup;
pub mod resize;
pub mod rules;
pub mod text;

use async_trait::async_trait;
use toss::{Frame, Size, WidthDb};

// TODO Add Error type and return Result-s (at least in Widget::render)

#[async_trait]
pub trait Widget {
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Size;

    async fn render(self: Box<Self>, frame: &mut Frame);
}

pub type BoxedWidget = Box<dyn Widget + Send + Sync>;

impl<W: 'static + Widget + Send + Sync> From<W> for BoxedWidget {
    fn from(widget: W) -> Self {
        Box::new(widget)
    }
}
