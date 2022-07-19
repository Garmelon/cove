//! Intermediate representation of chat history as blocks of things.

use std::collections::VecDeque;

use toss::styled::Styled;

use crate::macros::some_or_return;

pub enum MarkerBlock<I> {
    After(I),
    Bottom,
}

pub enum MsgContent {
    Msg { nick: Styled, lines: Vec<Styled> },
    Placeholder,
}

pub struct MsgBlock<I> {
    id: I,
    cursor: bool,
    content: MsgContent,
}

impl<I> MsgBlock<I> {
    pub fn height(&self) -> i32 {
        match &self.content {
            MsgContent::Msg { lines, .. } => lines.len() as i32,
            MsgContent::Placeholder => 1,
        }
    }
}

pub struct ComposeBlock {
    // TODO Editor widget
}

pub enum BlockBody<I> {
    Marker(MarkerBlock<I>),
    Msg(MsgBlock<I>),
    Compose(ComposeBlock),
}

pub struct Block<I> {
    line: i32,
    indent: usize,
    body: BlockBody<I>,
}

impl<I> Block<I> {
    pub fn height(&self) -> i32 {
        match &self.body {
            BlockBody::Marker(m) => 0,
            BlockBody::Msg(m) => m.height(),
            BlockBody::Compose(e) => todo!(),
        }
    }
}

/// Pre-layouted messages as a sequence of blocks.
///
/// These blocks are straightforward to render, but also provide a level of
/// abstraction between the layouting and actual displaying of messages.
///
/// The following equation describes the relationship between the
/// [`Blocks::top_line`] and [`Blocks::bottom_line`] fields:
///
/// `bottom_line - top_line = sum of all heights - 1`
///
/// This ensures that `top_line` is always the first line and `bottom_line` is
/// always the last line in a nonempty [`Blocks`]. In an empty layout, the
/// equation simplifies to
///
/// `bottom_line = top_line - 1`
pub struct Blocks<I> {
    pub blocks: VecDeque<Block<I>>,
    /// The top line of the first block. Useful for prepending blocks,
    /// especially to empty [`Blocks`]s.
    pub top_line: i32,
    /// The bottom line of the last block. Useful for appending blocks,
    /// especially to empty [`Blocks`]s.
    pub bottom_line: i32,
    /// The root of the first and last tree, if any. Useful for figuring out
    /// which blocks to prepend or append.
    pub roots: Option<(I, I)>,
}

impl<I> Blocks<I> {
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
            roots: None,
        }
    }

    pub fn find<F>(&self, f: F) -> Option<&Block<I>>
    where
        F: Fn(&Block<I>) -> bool,
    {
        self.blocks.iter().find(|b| f(b))
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: Fn(&mut Block<I>),
    {
        for block in &mut self.blocks {
            f(block);
        }
    }

    fn find_index_and_line<F>(&self, f: F) -> Option<(usize, i32)>
    where
        F: Fn(&Block<I>) -> Option<i32>,
    {
        self.blocks
            .iter()
            .enumerate()
            .find_map(|(i, b)| f(b).map(|l| (i, l)))
    }

    /// Update the offsets such that the line of the first block with a `Some`
    /// return value becomes that value.
    pub fn recalculate_offsets<F>(&mut self, f: F)
    where
        F: Fn(&Block<I>) -> Option<i32>,
    {
        let (idx, line) = some_or_return!(self.find_index_and_line(f));

        // Propagate lines from index to both ends
        self.blocks[idx].line = line;
        for i in (0..idx).rev() {
            self.blocks[i].line = self.blocks[i + 1].line - self.blocks[i].height();
        }
        for i in (idx + 1)..self.blocks.len() {
            self.blocks[i].line = self.blocks[i - 1].line + self.blocks[i - 1].height();
        }

        self.top_line = self.blocks.front().expect("blocks nonempty").line;
        let bottom = self.blocks.back().expect("blocks nonempty");
        self.bottom_line = bottom.line + bottom.height() - 1;
    }

    pub fn push_front(&mut self, mut block: Block<I>) {
        self.top_line -= block.height();
        block.line = self.top_line;
        self.blocks.push_front(block);
    }

    pub fn push_back(&mut self, mut block: Block<I>) {
        block.line = self.bottom_line + 1;
        self.bottom_line += block.height();
        self.blocks.push_back(block);
    }

    pub fn offset(&mut self, delta: i32) {
        self.top_line += delta;
        self.bottom_line += delta;
        for block in &mut self.blocks {
            block.line += delta;
        }
    }
}

impl<I: Ord> Blocks<I> {
    pub fn prepend(&mut self, mut layout: Self) {
        while let Some(block) = layout.blocks.pop_back() {
            self.push_front(block);
        }

        if let Some((l_root_top, l_root_bot)) = layout.roots {
            if let Some((root_top, root_bot)) = &mut self.roots {
                assert!(l_root_bot < *root_top);
                *root_top = l_root_top;
            } else {
                self.roots = Some((l_root_top, l_root_bot));
            }
        }
    }

    pub fn append(&mut self, mut layout: Self) {
        while let Some(block) = layout.blocks.pop_front() {
            self.push_back(block);
        }

        if let Some((l_root_top, l_root_bot)) = layout.roots {
            if let Some((root_top, root_bot)) = &mut self.roots {
                assert!(l_root_top > *root_bot);
                *root_bot = l_root_bot;
            } else {
                self.roots = Some((l_root_top, l_root_bot));
            }
        }
    }
}
