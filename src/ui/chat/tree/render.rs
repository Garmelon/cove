//! Rendering blocks to a [`Frame`].

use chrono::{DateTime, Utc};
use toss::frame::{Frame, Pos};
use toss::styled::Styled;

use crate::store::{Msg, MsgStore};

use super::blocks::{Block, BlockBody, MsgBlock, MsgContent};
use super::{util, InnerTreeViewState};

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn render_time(frame: &mut Frame, line: i32, time: Option<DateTime<Utc>>, is_cursor: bool) {
        let pos = Pos::new(0, line);
        let style = if is_cursor {
            util::style_time_inverted()
        } else {
            util::style_time()
        };

        if let Some(time) = time {
            let time = format!("{}", time.format(util::TIME_FORMAT));
            frame.write(pos, (&time, style));
        } else {
            frame.write(pos, (util::TIME_EMPTY, style));
        }
    }

    fn render_indent(frame: &mut Frame, line: i32, indent: usize, is_cursor: bool) {
        let pos = Pos::new(util::after_indent(0), line);
        let style = if is_cursor {
            util::style_indent_inverted()
        } else {
            util::style_indent()
        };

        let mut styled = Styled::default();
        for _ in 0..indent {
            styled = styled.then((util::INDENT, style));
        }

        frame.write(pos, styled);
    }

    fn render_nick(frame: &mut Frame, line: i32, indent: usize, nick: Styled) {
        let nick_pos = Pos::new(util::after_indent(indent), line);
        let styled = Styled::new("[").and_then(nick).then("]");
        frame.write(nick_pos, styled);
    }

    fn draw_msg_block(
        frame: &mut Frame,
        line: i32,
        time: Option<DateTime<Utc>>,
        indent: usize,
        msg: &MsgBlock<M::Id>,
        is_cursor: bool,
    ) {
        match &msg.content {
            MsgContent::Msg { nick, lines } => {
                let height: i32 = frame.size().height.into();
                let after_nick = util::after_nick(frame, indent, nick);

                for (i, text) in lines.iter().enumerate() {
                    let line = line + i as i32;
                    if line < 0 || line >= height {
                        continue;
                    }

                    if i == 0 {
                        Self::render_indent(frame, line, indent, is_cursor);
                        Self::render_time(frame, line, time, is_cursor);
                        Self::render_nick(frame, line, indent, nick.clone());
                    } else {
                        Self::render_indent(frame, line, indent + 1, false);
                        Self::render_indent(frame, line, indent, is_cursor);
                        Self::render_time(frame, line, None, is_cursor);
                    }

                    frame.write(Pos::new(after_nick, line), text.clone());
                }
            }
            MsgContent::Placeholder => {
                Self::render_time(frame, line, time, is_cursor);
                Self::render_indent(frame, line, indent, is_cursor);
                let pos = Pos::new(util::after_indent(indent), line);
                frame.write(pos, (util::PLACEHOLDER, util::style_placeholder()));
            }
        }
    }

    fn draw_block(frame: &mut Frame, block: &Block<M::Id>, is_cursor: bool) {
        match &block.body {
            BlockBody::Marker(_) => {}
            BlockBody::Msg(msg) => {
                Self::draw_msg_block(frame, block.line, block.time, block.indent, msg, is_cursor)
            }
            BlockBody::Compose(_) => {}
        }
    }

    pub fn draw_blocks(&self, frame: &mut Frame) {
        for block in self.last_blocks.iter() {
            Self::draw_block(frame, block, self.cursor.matches_block(block));
        }
    }
}
