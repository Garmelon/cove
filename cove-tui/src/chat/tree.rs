mod blocks;
mod constants;

use std::collections::VecDeque;
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{ContentStyle, Stylize};
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore, Tree};

use self::blocks::{Block, BlockContent, Blocks, MsgBlock};
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

    fn msg_to_block(
        &mut self,
        msg: &M,
        indent: usize,
        frame: &mut Frame,
        size: Size,
    ) -> Block<M::Id> {
        let nick = msg.nick();
        let content = msg.content();

        let used_width = TIME_WIDTH + INDENT_WIDTH * indent + 1 + frame.width(&nick) + 2;
        let rest_width = size.width as usize - used_width;

        let lines = toss::split_at_indices(&content, &frame.wrap(&content, rest_width));
        let lines = lines.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
        MsgBlock {
            time: msg.time(),
            nick,
            lines,
        }
        .into_block(msg.id(), indent)
    }

    fn layout_subtree(
        &mut self,
        tree: &Tree<M>,
        frame: &mut Frame,
        size: Size,
        indent: usize,
        id: &M::Id,
        layout: &mut Blocks<M::Id>,
    ) {
        let block = if let Some(msg) = tree.msg(id) {
            self.msg_to_block(msg, indent, frame, size)
        } else {
            Block::placeholder(id.clone(), indent)
        };
        layout.push_back(block);

        if let Some(children) = tree.children(id) {
            for child in children {
                self.layout_subtree(tree, frame, size, indent + 1, child, layout);
            }
        }
    }

    fn layout_tree(&mut self, tree: Tree<M>, frame: &mut Frame, size: Size) -> Blocks<M::Id> {
        let mut layout = Blocks::new();
        self.layout_subtree(&tree, frame, size, 0, tree.root(), &mut layout);
        layout
    }

    async fn expand_layout_upwards<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        layout: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while layout.top_line > 0 {
            let tree = store.tree(room, &tree_id).await;
            layout.prepend(self.layout_tree(tree, frame, size));
            if let Some(prev_tree_id) = store.prev_tree(room, &tree_id).await {
                tree_id = prev_tree_id;
            } else {
                break;
            }
        }
    }

    async fn expand_layout_downwards<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        layout: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while layout.bottom_line < size.height as i32 {
            let tree = store.tree(room, &tree_id).await;
            layout.append(self.layout_tree(tree, frame, size));
            if let Some(next_tree_id) = store.next_tree(room, &tree_id).await {
                tree_id = next_tree_id;
            } else {
                break;
            }
        }
    }

    async fn layout<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) -> Blocks<M::Id> {
        let height: i32 = size.height.into();
        if let Some(cursor) = cursor {
            // TODO Ensure focus lies on cursor path, otherwise unfocus
            // TODO Unfold all messages on path to cursor

            // Produce layout of cursor subtree (with correct offsets)
            let cursor_path = store.path(room, &cursor.id).await;
            let cursor_tree_id = cursor_path.first();
            let cursor_tree = store.tree(room, cursor_tree_id).await;
            let mut layout = self.layout_tree(cursor_tree, frame, size);
            layout.calculate_offsets_with_cursor(cursor, height);

            // Expand layout upwards and downwards
            // TODO Don't do this if there is a focus
            if let Some(prev_tree) = store.prev_tree(room, cursor_tree_id).await {
                self.expand_layout_upwards(room, store, frame, size, &mut layout, prev_tree)
                    .await;
            }
            if let Some(next_tree) = store.next_tree(room, cursor_tree_id).await {
                self.expand_layout_downwards(room, store, frame, size, &mut layout, next_tree)
                    .await;
            }

            layout
        } else {
            // TODO Ensure there is no focus

            // Start layout at the bottom of the screen
            let mut layout = Blocks::new_below(height - 1);

            // Expand layout upwards until the edge of the screen
            if let Some(last_tree) = store.last_tree(room).await {
                self.expand_layout_upwards(room, store, frame, size, &mut layout, last_tree)
                    .await;
            }

            layout
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
