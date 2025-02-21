use std::convert::Infallible;

use crossterm::style::Stylize;
use jiff::Zoned;
use toss::{
    Frame, Pos, Size, Style, Widget, WidgetExt, WidthDb,
    widgets::{Boxed, Empty, Text},
};

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

impl<E> Widget<E> for Indent {
    fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Result<Size, E> {
        let width = (INDENT_WIDTH * self.level).try_into().unwrap_or(u16::MAX);
        Ok(Size::new(width, 0))
    }

    fn draw(self, frame: &mut Frame) -> Result<(), E> {
        let size = frame.size();
        let indent_string = INDENT_STR.repeat(self.level);

        for y in 0..size.height {
            frame.write(Pos::new(0, y.into()), (&indent_string, self.style))
        }

        Ok(())
    }
}

const TIME_FORMAT: &str = "%Y-%m-%d %H:%M";
const TIME_WIDTH: u16 = 16;

pub struct Time(Boxed<'static, Infallible>);

impl Time {
    pub fn new(time: Option<Zoned>, style: Style) -> Self {
        let widget = if let Some(time) = time {
            let text = time.strftime(TIME_FORMAT).to_string();
            Text::new((text, style))
                .background()
                .with_style(style)
                .boxed()
        } else {
            Empty::new()
                .with_width(TIME_WIDTH)
                .background()
                .with_style(style)
                .boxed()
        };
        Self(widget)
    }
}

impl<E> Widget<E> for Time {
    fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        Ok(self.0.size(widthdb, max_width, max_height).infallible())
    }

    fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.0.draw(frame).infallible();
        Ok(())
    }
}

pub struct Seen(Boxed<'static, Infallible>);

impl Seen {
    pub fn new(seen: bool) -> Self {
        let widget = if seen {
            Empty::new().with_width(1).boxed()
        } else {
            let style = Style::new().black().on_green();
            Text::new("*").background().with_style(style).boxed()
        };
        Self(widget)
    }
}

impl<E> Widget<E> for Seen {
    fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        Ok(self.0.size(widthdb, max_width, max_height).infallible())
    }

    fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.0.draw(frame).infallible();
        Ok(())
    }
}
