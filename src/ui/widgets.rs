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
use toss::{AsyncWidget, Frame, Size, WidthDb};

use super::UiError;

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

/// Wrapper that implements [`Widget`] for an [`AsyncWidget`].
pub struct AsyncWidgetWrapper<I> {
    inner: I,
}

impl<I> AsyncWidgetWrapper<I> {
    pub fn new(inner: I) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<I> Widget for AsyncWidgetWrapper<I>
where
    I: AsyncWidget<UiError> + Send + Sync,
{
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Size {
        self.inner
            .size(widthdb, max_width, max_height)
            .await
            .unwrap()
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        self.inner.draw(frame).await.unwrap();
    }
}

/// Wrapper that implements [`AsyncWidget`] for a [`Widget`].
pub struct WidgetWrapper {
    inner: BoxedWidget,
}

impl WidgetWrapper {
    pub fn new<W: Into<BoxedWidget>>(inner: W) -> Self {
        Self {
            inner: inner.into(),
        }
    }
}

#[async_trait]
impl<E> AsyncWidget<E> for WidgetWrapper {
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        Ok(self.inner.size(widthdb, max_width, max_height).await)
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.inner.render(frame).await;
        Ok(())
    }
}
