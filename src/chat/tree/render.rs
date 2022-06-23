//! Rendering blocks to a [`Frame`].

use chrono::{DateTime, Utc};
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::store::Msg;

use super::blocks::{Block, BlockBody, Blocks};
use super::util::{
    self, style_indent, style_indent_inverted, style_placeholder, style_time, style_time_inverted,
    INDENT, PLACEHOLDER, TIME_EMPTY, TIME_FORMAT,
};
use super::TreeView;

fn render_time(frame: &mut Frame, x: i32, y: i32, cursor: bool, time: Option<DateTime<Utc>>) {
    let pos = Pos::new(x, y);

    let style = if cursor {
        style_time_inverted()
    } else {
        style_time()
    };

    if let Some(time) = time {
        let time = format!("{}", time.format(TIME_FORMAT));
        frame.write(pos, &time, style);
    } else {
        frame.write(pos, TIME_EMPTY, style);
    }
}

fn render_indent(frame: &mut Frame, x: i32, y: i32, cursor: bool, indent: usize) {
    for i in 0..indent {
        let pos = Pos::new(x + util::after_indent(i), y);

        let style = if cursor {
            style_indent_inverted()
        } else {
            style_indent()
        };

        frame.write(pos, INDENT, style);
    }
}

fn render_nick(frame: &mut Frame, x: i32, y: i32, indent: usize, nick: &str) {
    let nick_pos = Pos::new(x + util::after_indent(indent), y);
    let nick = format!("[{}]", nick);
    frame.write(nick_pos, &nick, ContentStyle::default());
}

fn render_block<M: Msg>(frame: &mut Frame, pos: Pos, size: Size, block: &Block<M::Id>) {
    match &block.body {
        BlockBody::Msg(msg) => {
            let after_nick = util::after_nick(frame, block.indent, &msg.nick);

            for (i, line) in msg.lines.iter().enumerate() {
                let y = pos.y + block.line + i as i32;
                if y < 0 || y >= size.height as i32 {
                    continue;
                }

                render_indent(frame, pos.x, y, block.cursor, block.indent);

                if i == 0 {
                    render_time(frame, pos.x, y, block.cursor, block.time);
                    render_nick(frame, pos.x, y, block.indent, &msg.nick);
                } else {
                    render_time(frame, pos.x, y, block.cursor, None);
                }

                let line_pos = Pos::new(pos.x + after_nick, y);
                frame.write(line_pos, line, ContentStyle::default());
            }
        }
        BlockBody::Placeholder => {
            let y = pos.y + block.line;
            render_time(frame, pos.x, y, block.cursor, block.time);
            render_indent(frame, pos.x, y, block.cursor, block.indent);
            let pos = Pos::new(pos.x + util::after_indent(block.indent), y);
            frame.write(pos, PLACEHOLDER, style_placeholder());
        }
    }
}

impl<M: Msg> TreeView<M> {
    pub fn render_blocks(frame: &mut Frame, pos: Pos, size: Size, layout: &Blocks<M::Id>) {
        for block in &layout.blocks {
            render_block::<M>(frame, pos, size, block);
        }
    }
}
