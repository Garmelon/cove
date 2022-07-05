//! Rendering blocks to a [`Frame`].

use chrono::{DateTime, Utc};
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;

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
        frame.write(pos, (&time, style));
    } else {
        frame.write(pos, (TIME_EMPTY, style));
    }
}

fn render_indent(frame: &mut Frame, x: i32, y: i32, cursor: bool, indent: usize) {
    let style = if cursor {
        style_indent_inverted()
    } else {
        style_indent()
    };

    let mut styled = Styled::default();
    for _ in 0..indent {
        styled = styled.then((INDENT, style));
    }

    frame.write(Pos::new(x + util::after_indent(0), y), styled);
}

fn render_nick(frame: &mut Frame, x: i32, y: i32, indent: usize, nick: Styled) {
    let nick_pos = Pos::new(x + util::after_indent(indent), y);
    let styled = Styled::new("[").and_then(nick).then("]");
    frame.write(nick_pos, styled);
}

fn render_block<M: Msg>(frame: &mut Frame, pos: Pos, size: Size, block: Block<M::Id>) {
    match block.body {
        BlockBody::Msg(msg) => {
            let after_nick = util::after_nick(frame, block.indent, &msg.nick.text());

            for (i, line) in msg.lines.into_iter().enumerate() {
                let y = pos.y + block.line + i as i32;
                if y < 0 || y >= pos.y + size.height as i32 {
                    continue;
                }

                if i == 0 {
                    render_indent(frame, pos.x, y, block.cursor, block.indent);
                    render_time(frame, pos.x, y, block.cursor, block.time);
                    render_nick(frame, pos.x, y, block.indent, msg.nick.clone());
                } else {
                    render_indent(frame, pos.x, y, false, block.indent + 1);
                    render_indent(frame, pos.x, y, block.cursor, block.indent);
                    render_time(frame, pos.x, y, block.cursor, None);
                }

                let line_pos = Pos::new(pos.x + after_nick, y);
                frame.write(line_pos, line);
            }
        }
        BlockBody::Placeholder => {
            let y = pos.y + block.line;
            render_time(frame, pos.x, y, block.cursor, block.time);
            render_indent(frame, pos.x, y, block.cursor, block.indent);
            let pos = Pos::new(pos.x + util::after_indent(block.indent), y);
            frame.write(pos, (PLACEHOLDER, style_placeholder()));
        }
    }
}

impl<M: Msg> TreeView<M> {
    pub fn render_blocks(frame: &mut Frame, pos: Pos, size: Size, layout: Blocks<M::Id>) {
        for block in layout.blocks {
            render_block::<M>(frame, pos, size, block);
        }
    }
}
