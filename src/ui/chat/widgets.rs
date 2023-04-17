use std::convert::Infallible;

use async_trait::async_trait;
use crossterm::style::Stylize;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;
use toss::widgets::{BoxedAsync, Empty, Text};
use toss::{AsyncWidget, Frame, Pos, Size, Style, WidgetExt, WidthDb};

use crate::util::InfallibleExt;

pub const INDENT_STR: &str = "│ ";
pub const INDENT_WIDTH: usize = 2;

pub struct Indent {
    level: usize,
    style: Style,
}

impl Indent {
    pub fn new(level: usize, style: Style) -> Self {
        Self { level, style }
    }
}

#[async_trait]
impl<E> AsyncWidget<E> for Indent {
    async fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Result<Size, E> {
        let width = (INDENT_WIDTH * self.level).try_into().unwrap_or(u16::MAX);
        Ok(Size::new(width, 0))
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), E> {
        let size = frame.size();
        let indent_string = INDENT_STR.repeat(self.level);

        for y in 0..size.height {
            frame.write(Pos::new(0, y.into()), (&indent_string, self.style))
        }

        Ok(())
    }
}

const TIME_FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day] [hour]:[minute]");
const TIME_WIDTH: u16 = 16;

pub struct Time(BoxedAsync<'static, Infallible>);

impl Time {
    pub fn new(time: Option<OffsetDateTime>, style: Style) -> Self {
        let widget = if let Some(time) = time {
            let text = time.format(TIME_FORMAT).expect("could not format time");
            Text::new((text, style))
                .background()
                .with_style(style)
                .boxed_async()
        } else {
            Empty::new()
                .with_width(TIME_WIDTH)
                .background()
                .with_style(style)
                .boxed_async()
        };
        Self(widget)
    }
}

#[async_trait]
impl<E> AsyncWidget<E> for Time {
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        Ok(self
            .0
            .size(widthdb, max_width, max_height)
            .await
            .infallible())
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.0.draw(frame).await.infallible();
        Ok(())
    }
}

pub struct Seen(BoxedAsync<'static, Infallible>);

impl Seen {
    pub fn new(seen: bool) -> Self {
        let widget = if seen {
            Empty::new().with_width(1).boxed_async()
        } else {
            let style = Style::new().black().on_green();
            Text::new("*").background().with_style(style).boxed_async()
        };
        Self(widget)
    }
}

#[async_trait]
impl<E> AsyncWidget<E> for Seen {
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        Ok(self
            .0
            .size(widthdb, max_width, max_height)
            .await
            .infallible())
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.0.draw(frame).await.infallible();
        Ok(())
    }
}
