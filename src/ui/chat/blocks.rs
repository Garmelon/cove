use std::collections::{vec_deque, VecDeque};

use toss::frame::Frame;

use crate::macros::some_or_return;
use crate::ui::widgets::BoxedWidget;

pub struct Block<I> {
    id: I,
    top_line: i32,
    height: i32,
    widget: BoxedWidget,
}

impl<I> Block<I> {
    pub fn new<W: Into<BoxedWidget>>(frame: &mut Frame, width: u16, id: I, widget: W) -> Self {
        // Interestingly, rust-analyzer fails to deduce the type of `widget`
        // here but rustc knows it's a `BoxedWidget`.
        let widget = widget.into();
        let size = widget.size(frame, Some(width), None);
        Self {
            id,
            top_line: 0,
            height: size.height.into(),
            widget,
        }
    }
}

pub struct Blocks<I> {
    pub blocks: VecDeque<Block<I>>,
    /// The top line of the first block. Useful for prepending blocks,
    /// especially to empty [`Blocks`]s.
    pub top_line: i32,
    /// The bottom line of the last block. Useful for appending blocks,
    /// especially to empty [`Blocks`]s.
    pub bottom_line: i32,
}

impl<I> Blocks<I> {
    pub fn new() -> Self {
        Self::new_below(0)
    }

    /// Create a new [`Blocks`] such that the first prepended line will be on
    /// `line`.
    pub fn new_below(line: i32) -> Self {
        Self {
            blocks: VecDeque::new(),
            top_line: line + 1,
            bottom_line: line,
        }
    }

    pub fn offset(&mut self, delta: i32) {
        self.top_line += delta;
        self.bottom_line += delta;
        for block in &mut self.blocks {
            block.top_line += delta;
        }
    }

    pub fn push_front(&mut self, mut block: Block<I>) {
        self.top_line -= block.height;
        block.top_line = self.top_line;
        self.blocks.push_front(block);
    }

    pub fn push_back(&mut self, mut block: Block<I>) {
        block.top_line = self.bottom_line + 1;
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

impl<I: Eq> Blocks<I> {
    pub fn recalculate_offsets(&mut self, id: I, top_line: i32) {
        let idx = some_or_return!(self
            .blocks
            .iter()
            .enumerate()
            .find(|(_, b)| b.id == id)
            .map(|(i, _)| i));

        self.blocks[idx].top_line = top_line;

        // Propagate changes to top
        for i in (0..idx).rev() {
            self.blocks[i].top_line = self.blocks[i + 1].top_line - self.blocks[i].height;
        }
        self.top_line = self.blocks.front().expect("blocks nonempty").top_line;

        // Propagate changes to bottom
        for i in (idx + 1)..self.blocks.len() {
            self.blocks[i].top_line = self.blocks[i - 1].top_line + self.blocks[i - 1].height;
        }
        let bottom = self.blocks.back().expect("blocks nonempty");
        self.bottom_line = bottom.top_line + bottom.height - 1;
    }
}
