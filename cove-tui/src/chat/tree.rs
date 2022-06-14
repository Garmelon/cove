mod blocks;
mod constants;
mod layout;

use std::marker::PhantomData;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{ContentStyle, Stylize};
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore};

use self::blocks::{BlockContent, Blocks};
use self::constants::{INDENT, INDENT_WIDTH, TIME_WIDTH};

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

    fn render_indentation(&mut self, frame: &mut Frame, pos: Pos, indent: usize, cursor: bool) {
        for i in 0..indent {
            let x = TIME_WIDTH + INDENT_WIDTH * i;
            let pos = Pos::new(pos.x + x as i32, pos.y);
            let style = if cursor {
                ContentStyle::default().black().on_white()
            } else {
                ContentStyle::default()
            };
            frame.write(pos, INDENT, style);
        }
    }

    fn render_layout(&mut self, frame: &mut Frame, pos: Pos, size: Size, layout: &Blocks<M::Id>) {
        for block in &layout.blocks {
            match &block.content {
                BlockContent::Msg(msg) => {
                    let nick_width = frame.width(&msg.nick) as i32;
                    for (i, line) in msg.lines.iter().enumerate() {
                        let y = pos.y + block.line + i as i32;
                        if y < 0 || y >= size.height as i32 {
                            continue;
                        }

                        self.render_indentation(
                            frame,
                            Pos::new(pos.x, y),
                            block.indent,
                            block.cursor,
                        );
                        let after_indent =
                            pos.x + (TIME_WIDTH + INDENT_WIDTH * block.indent) as i32;
                        if i == 0 {
                            let time = format!("{}", msg.time.format("%H:%M"));
                            frame.write(Pos::new(pos.x, y), &time, ContentStyle::default());
                            let nick = format!("[{}]", msg.nick);
                            frame.write(Pos::new(after_indent, y), &nick, ContentStyle::default());
                        }
                        let msg_x = after_indent + 1 + nick_width + 2;
                        frame.write(Pos::new(msg_x, y), line, ContentStyle::default());
                    }
                }
                BlockContent::Placeholder => {
                    self.render_indentation(frame, pos, block.indent, block.cursor);
                    let x = pos.x + (TIME_WIDTH + INDENT_WIDTH * block.indent) as i32;
                    let y = pos.y + block.line;
                    frame.write(Pos::new(x, y), "[...]", ContentStyle::default());
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
