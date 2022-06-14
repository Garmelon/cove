//! Intermediate representation of messages as blocks of lines.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};

use crate::chat::Cursor;

use super::util;

pub struct Block<I> {
    pub id: I,
    pub line: i32,
    pub height: i32,
    pub cursor: bool,
    pub time: Option<DateTime<Utc>>,
    pub indent: usize,
    pub body: BlockBody,
}

impl<I> Block<I> {
    pub fn msg(
        id: I,
        indent: usize,
        time: DateTime<Utc>,
        nick: String,
        lines: Vec<String>,
    ) -> Self {
        Self {
            id,
            line: 0,
            height: lines.len() as i32,
            indent,
            time: Some(time),
            cursor: false,
            body: BlockBody::Msg(MsgBlock { nick, lines }),
        }
    }

    pub fn placeholder(id: I, indent: usize) -> Self {
        Self {
            id,
            line: 0,
            height: 1,
            indent,
            time: None,
            cursor: false,
            body: BlockBody::Placeholder,
        }
    }

    pub fn time(mut self, time: DateTime<Utc>) -> Self {
        self.time = Some(time);
        self
    }
}
pub enum BlockBody {
    Msg(MsgBlock),
    Placeholder,
}

pub struct MsgBlock {
    pub nick: String,
    pub lines: Vec<String>,
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

    pub fn calculate_offsets_with_cursor(&mut self, cursor: &Cursor<I>, height: u16) {
        let cursor_index = self.mark_cursor(&cursor.id);
        let cursor_line = util::proportion_to_line(height, cursor.proportion);

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
