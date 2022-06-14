mod blocks;
mod constants;
mod layout;

use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{ContentStyle, Stylize};
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore};

use self::blocks::{BlockBody, Blocks};
use self::constants::{
    after_indent, style_indent, style_indent_inverted, style_placeholder, style_time,
    style_time_inverted, INDENT, INDENT_WIDTH, PLACEHOLDER, TIME_EMPTY, TIME_FORMAT, TIME_WIDTH,
};

use super::Cursor;

pub struct TreeView<M: Msg> {
    // pub focus: Option<M::Id>,
    // pub folded: HashSet<M::Id>,
    // pub minimized: HashSet<M::Id>,
    phantom: PhantomData<M::Id>, // TODO Remove
}

impl<M: Msg> TreeView<M> {
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }

    fn render_time(frame: &mut Frame, x: i32, y: i32, time: Option<DateTime<Utc>>, cursor: bool) {
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

    fn render_indent(frame: &mut Frame, x: i32, y: i32, indent: usize, cursor: bool) {
        for i in 0..indent {
            let pos = Pos::new(x + after_indent(i), y);

            let style = if cursor {
                style_indent_inverted()
            } else {
                style_indent()
            };

            frame.write(pos, INDENT, style);
        }
    }

    fn render_layout(&mut self, frame: &mut Frame, pos: Pos, size: Size, layout: &Blocks<M::Id>) {
        for block in &layout.blocks {
            // Draw rest of block
            match &block.body {
                BlockBody::Msg(msg) => {
                    let nick_width = frame.width(&msg.nick) as i32;
                    for (i, line) in msg.lines.iter().enumerate() {
                        let y = pos.y + block.line + i as i32;
                        if y < 0 || y >= size.height as i32 {
                            continue;
                        }

                        Self::render_indent(frame, pos.x, y, block.indent, block.cursor);
                        let after_indent =
                            pos.x + (TIME_WIDTH + INDENT_WIDTH * block.indent) as i32;
                        if i == 0 {
                            Self::render_time(frame, pos.x, y, block.time, block.cursor);
                            let nick = format!("[{}]", msg.nick);
                            frame.write(Pos::new(after_indent, y), &nick, ContentStyle::default());
                        } else {
                            Self::render_time(frame, pos.x, y, None, block.cursor);
                        }
                        let msg_x = after_indent + 1 + nick_width + 2;
                        frame.write(Pos::new(msg_x, y), line, ContentStyle::default());
                    }
                }
                BlockBody::Placeholder => {
                    let y = pos.y + block.line;
                    Self::render_time(frame, pos.x, y, block.time, block.cursor);
                    Self::render_indent(frame, pos.x, y, block.indent, block.cursor);
                    let pos = Pos::new(pos.x + after_indent(block.indent), y);
                    frame.write(pos, PLACEHOLDER, style_placeholder());
                }
            }
        }
    }

    async fn move_to_prev_msg<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
    ) {
        let tree = if let Some(cursor) = cursor {
            let path = store.path(room, &cursor.id).await;
            let tree = store.tree(room, path.first()).await;
            if let Some(prev_sibling) = tree.prev_sibling(&cursor.id) {
                cursor.id = prev_sibling.clone();
                return;
            } else if let Some(parent) = tree.parent(&cursor.id) {
                cursor.id = parent;
                return;
            } else {
                store.prev_tree(room, path.first()).await
            }
        } else {
            store.last_tree(room).await
        };

        if let Some(tree) = tree {
            let tree = store.tree(room, &tree).await;
            let cursor_id = tree.last_child(tree.root().clone());
            if let Some(cursor) = cursor {
                cursor.id = cursor_id;
            } else {
                *cursor = Some(Cursor {
                    id: cursor_id,
                    proportion: 1.0,
                });
            }
        }
    }

    async fn center_cursor(&mut self, cursor: &mut Option<Cursor<M::Id>>) {
        if let Some(cursor) = cursor {
            cursor.proportion = 0.5;
        }
    }

    pub async fn handle_key_event<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
        event: KeyEvent,
        frame: &mut Frame,
        size: Size,
    ) {
        match event.code {
            KeyCode::Char('k') => self.move_to_prev_msg(store, room, cursor).await,
            KeyCode::Char('z') => self.center_cursor(cursor).await,
            _ => {}
        }
    }

    pub async fn render<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        let layout = self.layout(room, store, cursor, frame, size).await;
        self.render_layout(frame, pos, size, &layout);
    }
}
