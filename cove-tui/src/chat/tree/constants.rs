use crossterm::style::{ContentStyle, Stylize};

pub const TIME_FORMAT: &str = "%H:%M ";
pub const TIME_WIDTH: usize = 6;

pub fn style_time() -> ContentStyle {
    ContentStyle::default().grey()
}

pub fn style_time_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub const INDENT: &str = "│ ";
pub const INDENT_WIDTH: usize = 2;

pub fn style_indent() -> ContentStyle {
    ContentStyle::default().grey()
}

pub fn style_indent_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}
