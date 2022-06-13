use std::collections::VecDeque;
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore, Tree};

use super::Cursor;

const TIME_WIDTH: usize = 5; // hh:mm
const INDENT: &str = "| ";
const INDENT_WIDTH: usize = 2;

struct Block<I> {
    line: i32,
    height: i32,
    id: Option<I>,
    indent: usize,
    cursor: bool,
    content: BlockContent,
}

impl<I> Block<I> {
    fn placeholder(id: I, indent: usize) -> Self {
        Self {
            line: 0,
            height: 1,
            id: Some(id),
            indent,
            cursor: false,
            content: BlockContent::Placeholder,
        }
    }
}

enum BlockContent {
    Msg(MsgBlock),
    Placeholder,
}

struct MsgBlock {
    time: DateTime<Utc>,
    nick: String,
    lines: Vec<String>,
}

impl MsgBlock {
    fn to_block<I>(self, id: I, indent: usize) -> Block<I> {
        Block {
            line: 0,
            height: self.lines.len() as i32,
            id: Some(id),
            indent,
            cursor: false,
            content: BlockContent::Msg(self),
        }
    }
}

/// Pre-layouted messages as a sequence of blocks.
///
/// These blocks are straightforward to render, but also provide a level of
/// abstraction between the layouting and actual displaying of messages. This
/// might be useful in the future to ensure the cursor is always on a visible
/// message, for example.
///
/// The following equation describes the relationship between the
/// [`Layout::top_line`] and [`Layout::bottom_line`] fields:
///
/// `bottom_line - top_line + 1 = sum of all heights`
///
/// This ensures that `top_line` is always the first line and `bottom_line` is
/// always the last line in a nonempty [`Layout`]. In an empty layout, the
/// equation simplifies to
///
/// `top_line = bottom_line + 1`
struct Layout<I> {
    blocks: VecDeque<Block<I>>,
    /// The top line of the first block. Useful for prepending blocks,
    /// especially to empty [`Layout`]s.
    top_line: i32,
    /// The bottom line of the last block. Useful for appending blocks,
    /// especially to empty [`Layout`]s.
    bottom_line: i32,
}

impl<I: PartialEq> Layout<I> {
    fn new() -> Self {
        Self::new_below(0)
    }

    /// Create a new [`Layout`] such that prepending a single line will result
    /// in `top_line = bottom_line = line`.
    fn new_below(line: i32) -> Self {
        Self {
            blocks: VecDeque::new(),
            top_line: line + 1,
            bottom_line: line,
        }
    }

    fn mark_cursor(&mut self, id: &I) -> usize {
        let mut cursor = None;
        for (i, block) in self.blocks.iter_mut().enumerate() {
            if block.id.as_ref() == Some(id) {
                block.cursor = true;
                if cursor.is_some() {
                    panic!("more than one cursor in layout");
                }
                cursor = Some(i);
            }
        }
        cursor.expect("no cursor in layout")
    }

    fn calculate_offsets_with_cursor(&mut self, cursor: &Cursor<I>, height: i32) {
        let cursor_index = self.mark_cursor(&cursor.id);
        let cursor_line = ((height - 1) as f32 * cursor.proportion).round() as i32;

        // Propagate lines from cursor to both ends
        self.blocks[cursor_index].line = cursor_line;
        for i in (0..cursor_index).rev() {
            // let succ_line = self.0[i + 1].line;
            // let curr = &mut self.0[i];
            // curr.line = succ_line - curr.height;
            self.blocks[i].line = self.blocks[i + 1].line - self.blocks[i].height;
        }
        for i in (cursor_index + 1)..self.blocks.len() {
            // let pred = &self.0[i - 1];
            // self.0[i].line = pred.line + pred.height;
            self.blocks[i].line = self.blocks[i - 1].line + self.blocks[i - 1].height;
        }
        self.top_line = self.blocks.front().expect("blocks nonempty").line;
        let bottom = self.blocks.back().expect("blocks nonempty");
        self.bottom_line = bottom.line + bottom.height - 1;
    }

    fn push_front(&mut self, mut block: Block<I>) {
        self.top_line -= block.height;
        block.line = self.top_line;
        self.blocks.push_front(block);
    }

    fn push_back(&mut self, mut block: Block<I>) {
        block.line = self.bottom_line + 1;
        self.bottom_line += block.height;
        self.blocks.push_back(block);
    }

    fn prepend(&mut self, mut layout: Self) {
        while let Some(mut block) = layout.blocks.pop_back() {
            self.push_front(block);
        }
    }

    fn append(&mut self, mut layout: Self) {
        while let Some(mut block) = layout.blocks.pop_front() {
            self.push_back(block);
        }
    }
}

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

        let used_width = TIME_WIDTH + 1 + INDENT_WIDTH * indent + 1 + frame.width(&nick) + 2;
        let rest_width = size.width as usize - used_width;

        let lines = toss::split_at_indices(&content, &frame.wrap(&content, rest_width));
        let lines = lines.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
        MsgBlock {
            time: msg.time(),
            nick,
            lines,
        }
        .to_block(msg.id(), indent)
    }

    fn layout_subtree(
        &mut self,
        tree: &Tree<M>,
        frame: &mut Frame,
        size: Size,
        indent: usize,
        id: &M::Id,
        layout: &mut Layout<M::Id>,
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

    fn layout_tree(&mut self, tree: Tree<M>, frame: &mut Frame, size: Size) -> Layout<M::Id> {
        let mut layout = Layout::new();
        self.layout_subtree(&tree, frame, size, 0, tree.root(), &mut layout);
        layout
    }

    async fn expand_layout_upwards<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        layout: &mut Layout<M::Id>,
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
        layout: &mut Layout<M::Id>,
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
    ) -> Layout<M::Id> {
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
            let mut layout = Layout::new_below(height);

            // Expand layout upwards until the edge of the screen
            if let Some(last_tree) = store.last_tree(room).await {
                self.expand_layout_upwards(room, store, frame, size, &mut layout, last_tree)
                    .await;
            }

            layout
        }
    }

    fn render_indentation(&mut self, frame: &mut Frame, pos: Pos, indent: usize) {
        for i in 0..indent {
            let x = TIME_WIDTH + 1 + INDENT_WIDTH * i;
            let pos = Pos::new(pos.x + x as i32, pos.y);
            frame.write(pos, INDENT, ContentStyle::default());
        }
    }

    fn render_layout(&mut self, frame: &mut Frame, pos: Pos, size: Size, layout: &Layout<M::Id>) {
        for block in &layout.blocks {
            match &block.content {
                BlockContent::Msg(msg) => {
                    let time = format!("{}", msg.time.format("%h:%m"));
                    frame.write(pos, &time, ContentStyle::default());

                    let nick_width = frame.width(&msg.nick) as i32;
                    for (i, line) in msg.lines.iter().enumerate() {
                        let y = pos.y + block.line + i as i32;
                        if y < 0 || y >= size.height as i32 {
                            continue;
                        }

                        self.render_indentation(frame, Pos::new(pos.x, y), block.indent);
                        let after_indentation =
                            pos.x + (TIME_WIDTH + 1 + INDENT_WIDTH * block.indent) as i32;
                        if i == 0 {
                            let nick_x = after_indentation;
                            let nick = format!("[{}]", msg.nick);
                            frame.write(Pos::new(nick_x, y), &nick, ContentStyle::default());
                        }
                        let msg_x = after_indentation + 1 + nick_width + 2;
                        frame.write(Pos::new(msg_x, y), line, ContentStyle::default());
                    }
                }
                BlockContent::Placeholder => {
                    self.render_indentation(frame, pos, block.indent);
                    let x = pos.x + (TIME_WIDTH + 1 + INDENT_WIDTH * block.indent) as i32;
                    let y = pos.y + block.line;
                    frame.write(Pos::new(x, y), "[...]", ContentStyle::default());
                }
            }
        }
    }

    pub fn handle_key_event<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
        event: KeyEvent,
        frame: &mut Frame,
        size: Size,
    ) {
        // TODO
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
