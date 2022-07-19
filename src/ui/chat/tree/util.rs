//! Constants and helper functions.

use crossterm::style::{ContentStyle, Stylize};
use toss::frame::Frame;

pub const TIME_FORMAT: &str = "%F %R ";
pub const TIME_EMPTY: &str = "                 ";
pub const TIME_WIDTH: usize = TIME_EMPTY.len();

pub fn style_time() -> ContentStyle {
    ContentStyle::default().grey()
}

pub fn style_time_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub const INDENT: &str = "│ ";
pub const INDENT_WIDTH: usize = INDENT.len();

pub fn style_indent() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

pub fn style_indent_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub const PLACEHOLDER: &str = "[...]";

pub fn style_placeholder() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

pub const MIN_CONTENT_WIDTH: usize = "[+, 1234 more]".len();

pub fn after_indent(indent: usize) -> i32 {
    (TIME_WIDTH + indent * INDENT_WIDTH) as i32
}

pub fn after_nick(frame: &mut Frame, indent: usize, nick: &str) -> i32 {
    after_indent(indent) + 1 + frame.width(nick) as i32 + 2
}
