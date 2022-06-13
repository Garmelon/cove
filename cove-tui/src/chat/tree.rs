use std::collections::VecDeque;
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore, Tree};

use super::Cursor;

struct Block<I> {
    line: i32,
    height: i32,
    id: Option<I>,
    cursor: bool,
    content: BlockContent,
}

enum BlockContent {
    Msg(MsgBlock),
    Placeholder,
}

struct MsgBlock {
    time: DateTime<Utc>,
    indent: usize,
    nick: String,
    content: Vec<String>,
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

    fn prepend(&mut self, mut layout: Self) {
        while let Some(mut block) = layout.blocks.pop_back() {
            self.top_line -= block.height;
            block.line = self.top_line;
            self.blocks.push_front(block);
        }
    }

    fn append(&mut self, mut layout: Self) {
        while let Some(mut block) = layout.blocks.pop_front() {
            block.line = self.bottom_line + 1;
            self.bottom_line += block.height;
            self.blocks.push_back(block);
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

    fn layout_tree(tree: Tree<M>) -> Layout<M::Id> {
        todo!()
    }

    async fn layout<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: S,
        cursor: &Option<Cursor<M::Id>>,
        size: Size,
    ) -> Layout<M::Id> {
        let height: i32 = size.height.into();
        if let Some(cursor) = cursor {
            // TODO Ensure focus lies on cursor path, otherwise unfocus
            // TODO Unfold all messages on path to cursor

            // Produce layout of cursor subtree (with correct offsets)
            let cursor_path = store.path(room, &cursor.id).await;
            let cursor_tree = store.tree(room, cursor_path.first()).await;
            let mut layout = Self::layout_tree(cursor_tree);
            layout.calculate_offsets_with_cursor(cursor, height);

            // TODO Expand layout upwards and downwards if there is no focus
            todo!()
        } else {
            // TODO Ensure there is no focus

            // Start layout at the bottom of the screen
            let mut layout = Layout::new_below(height);

            // Expand layout upwards until the edge of the screen
            let mut tree_id = store.last_tree(room).await;
            while layout.top_line > 0 {
                if let Some(actual_tree_id) = &tree_id {
                    let tree = store.tree(room, actual_tree_id).await;
                    layout.prepend(Self::layout_tree(tree));
                    tree_id = store.prev_tree(room, actual_tree_id).await;
                } else {
                    break;
                }
            }

            layout
        }
    }

    pub fn handle_key_event<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
        event: KeyEvent,
        size: Size,
    ) {
        // TODO
    }

    pub fn render<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        // TODO
        frame.write(Pos::new(0, 0), "Hello world!", ContentStyle::default());
    }
}
