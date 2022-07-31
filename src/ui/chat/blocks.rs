use std::collections::VecDeque;
use std::ops::Range;

use toss::frame::Frame;

use crate::macros::some_or_return;
use crate::ui::widgets::BoxedWidget;

pub struct Block<I> {
    pub id: I,
    pub top_line: i32,
    pub height: i32,
    /// The lines of the block that should be made visible if the block is
    /// focused on. By default, the focus encompasses the entire block.
    ///
    /// If not all of these lines can be made visible, the top of the range
    /// should be preferred over the bottom.
    pub focus: Range<i32>,
    pub widget: BoxedWidget,
}

impl<I> Block<I> {
    pub fn new<W: Into<BoxedWidget>>(frame: &mut Frame, id: I, widget: W) -> Self {
        // Interestingly, rust-analyzer fails to deduce the type of `widget`
        // here but rustc knows it's a `BoxedWidget`.
        let widget = widget.into();
        let size = widget.size(frame, Some(frame.size().width), None);
        let height = size.height.into();
        Self {
            id,
            top_line: 0,
            height,
            focus: 0..height,
            widget,
        }
    }

    pub fn focus(mut self, focus: Range<i32>) -> Self {
        self.focus = focus;
        self
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

    pub fn set_top_line(&mut self, line: i32) {
        self.top_line = line;

        if let Some(first_block) = self.blocks.front_mut() {
            first_block.top_line = self.top_line;
        }

        for i in 1..self.blocks.len() {
            self.blocks[i].top_line = self.blocks[i - 1].top_line + self.blocks[i - 1].height;
        }

        self.bottom_line = self
            .blocks
            .back()
            .map(|b| b.top_line + b.height - 1)
            .unwrap_or(self.top_line - 1);
    }

    pub fn set_bottom_line(&mut self, line: i32) {
        self.bottom_line = line;

        if let Some(last_block) = self.blocks.back_mut() {
            last_block.top_line = self.bottom_line + 1 - last_block.height;
        }

        for i in (1..self.blocks.len()).rev() {
            self.blocks[i - 1].top_line = self.blocks[i].top_line - self.blocks[i - 1].height;
        }

        self.top_line = self
            .blocks
            .front()
            .map(|b| b.top_line)
            .unwrap_or(self.bottom_line + 1)
    }
}

impl<I: Eq> Blocks<I> {
    pub fn find(&self, id: &I) -> Option<&Block<I>> {
        self.blocks.iter().find(|b| b.id == *id)
    }

    pub fn recalculate_offsets(&mut self, id: &I, top_line: i32) {
        let idx = some_or_return!(self
            .blocks
            .iter()
            .enumerate()
            .find(|(_, b)| b.id == *id)
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
