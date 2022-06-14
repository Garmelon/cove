use std::collections::VecDeque;

use chrono::{DateTime, Utc};

use crate::chat::Cursor;

pub struct Block<I> {
    pub line: i32,
    pub height: i32,
    pub id: I,
    pub indent: usize,
    pub cursor: bool,
    pub content: BlockContent,
}

impl<I> Block<I> {
    pub fn placeholder(id: I, indent: usize) -> Self {
        Self {
            line: 0,
            height: 1,
            id,
            indent,
            cursor: false,
            content: BlockContent::Placeholder,
        }
    }
}
pub enum BlockContent {
    Msg(MsgBlock),
    Placeholder,
}

pub struct MsgBlock {
    pub time: DateTime<Utc>,
    pub nick: String,
    pub lines: Vec<String>,
}

impl MsgBlock {
    pub fn into_block<I>(self, id: I, indent: usize) -> Block<I> {
        Block {
            line: 0,
            height: self.lines.len() as i32,
            id,
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
/// [`Blocks::top_line`] and [`Blocks::bottom_line`] fields:
///
/// `bottom_line - top_line + 1 = sum of all heights`
///
/// This ensures that `top_line` is always the first line and `bottom_line` is
/// always the last line in a nonempty [`Blocks`]. In an empty layout, the
/// equation simplifies to
///
/// `top_line = bottom_line + 1`
pub struct Blocks<I> {
    pub blocks: VecDeque<Block<I>>,
    /// The top line of the first block. Useful for prepending blocks,
    /// especially to empty [`Blocks`]s.
    pub top_line: i32,
    /// The bottom line of the last block. Useful for appending blocks,
    /// especially to empty [`Blocks`]s.
    pub bottom_line: i32,
}

impl<I: PartialEq> Blocks<I> {
    pub fn new() -> Self {
        Self::new_below(0)
    }

    /// Create a new [`Blocks`] such that prepending a single line will result
    /// in `top_line = bottom_line = line`.
    pub fn new_below(line: i32) -> Self {
        Self {
            blocks: VecDeque::new(),
            top_line: line + 1,
            bottom_line: line,
        }
    }

    fn mark_cursor(&mut self, id: &I) -> usize {
        let mut cursor = None;
        for (i, block) in self.blocks.iter_mut().enumerate() {
            if &block.id == id {
                block.cursor = true;
                if cursor.is_some() {
                    panic!("more than one cursor in layout");
                }
                cursor = Some(i);
            }
        }
        cursor.expect("no cursor in layout")
    }

    pub fn calculate_offsets_with_cursor(&mut self, cursor: &Cursor<I>, height: i32) {
        let cursor_index = self.mark_cursor(&cursor.id);
        let cursor_line = ((height - 1) as f32 * cursor.proportion).floor() as i32;

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

    pub fn push_front(&mut self, mut block: Block<I>) {
        self.top_line -= block.height;
        block.line = self.top_line;
        self.blocks.push_front(block);
    }

    pub fn push_back(&mut self, mut block: Block<I>) {
        block.line = self.bottom_line + 1;
        self.bottom_line += block.height;
        self.blocks.push_back(block);
    }

    pub fn prepend(&mut self, mut layout: Self) {
        while let Some(block) = layout.blocks.pop_back() {
            self.push_front(block);
        }
    }

    pub fn append(&mut self, mut layout: Self) {
        while let Some(block) = layout.blocks.pop_front() {
            self.push_back(block);
        }
    }
}
