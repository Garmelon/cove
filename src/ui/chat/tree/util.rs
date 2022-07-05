//! Constants and helper functions.

use crossterm::style::{ContentStyle, Stylize};
use toss::frame::Frame;

pub const TIME_FORMAT: &str = "%H:%M ";
pub const TIME_EMPTY: &str = "      ";
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
    ContentStyle::default().dark_grey()
}

pub fn style_indent_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub const PLACEHOLDER: &str = "[...]";

pub fn style_placeholder() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

// Something like this should fit: [+, 1234 more]
pub const MIN_CONTENT_WIDTH: usize = 14;

pub fn after_indent(indent: usize) -> i32 {
    (TIME_WIDTH + indent * INDENT_WIDTH) as i32
}

pub fn after_nick(frame: &mut Frame, indent: usize, nick: &str) -> i32 {
    after_indent(indent) + 1 + frame.width(nick) as i32 + 2
}

pub fn proportion_to_line(height: u16, proportion: f32) -> i32 {
    ((height - 1) as f32 * proportion).round() as i32
}

pub fn line_to_proportion(height: u16, line: i32) -> f32 {
    if height > 1 {
        line as f32 / (height - 1) as f32
    } else {
        0.0
    }
}
