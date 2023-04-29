//! Common rendering logic.

use std::collections::{vec_deque, VecDeque};

use toss::widgets::Predrawn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Range<T> {
    pub top: T,
    pub bottom: T,
}

impl<T> Range<T> {
    pub fn new(top: T, bottom: T) -> Self {
        Self { top, bottom }
    }
}

impl Range<i32> {
    pub fn shifted(self, delta: i32) -> Self {
        Self::new(self.top + delta, self.bottom + delta)
    }

    pub fn with_top(self, top: i32) -> Self {
        self.shifted(top - self.top)
    }

    pub fn with_bottom(self, bottom: i32) -> Self {
        self.shifted(bottom - self.bottom)
    }
}

pub struct Block<Id> {
    id: Id,
    widget: Predrawn,
    focus: Range<i32>,
    can_be_cursor: bool,
}

impl<Id> Block<Id> {
    pub fn new(id: Id, widget: Predrawn, can_be_cursor: bool) -> Self {
        let height: i32 = widget.size().height.into();
        Self {
            id,
            widget,
            focus: Range::new(0, height),
            can_be_cursor,
        }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn into_widget(self) -> Predrawn {
        self.widget
    }

    fn height(&self) -> i32 {
        self.widget.size().height.into()
    }

    pub fn set_focus(&mut self, focus: Range<i32>) {
        assert!(0 <= focus.top);
        assert!(focus.top <= focus.bottom);
        assert!(focus.bottom <= self.height());
        self.focus = focus;
    }

    pub fn focus(&self, range: Range<i32>) -> Range<i32> {
        Range::new(range.top + self.focus.top, range.top + self.focus.bottom)
    }

    pub fn can_be_cursor(&self) -> bool {
        self.can_be_cursor
    }
}

pub struct Blocks<Id> {
    blocks: VecDeque<Block<Id>>,
    range: Range<i32>,
    end: Range<bool>,
}

impl<Id> Blocks<Id> {
    pub fn new(at: i32) -> Self {
        Self {
            blocks: VecDeque::new(),
            range: Range::new(at, at),
            end: Range::new(false, false),
        }
    }

    pub fn range(&self) -> Range<i32> {
        self.range
    }

    pub fn end(&self) -> Range<bool> {
        self.end
    }

    pub fn iter(&self) -> Iter<'_, Id> {
        Iter {
            iter: self.blocks.iter(),
            range: self.range,
        }
    }

    pub fn into_iter(self) -> IntoIter<Id> {
        IntoIter {
            iter: self.blocks.into_iter(),
            range: self.range,
        }
    }

    pub fn find_block(&self, id: &Id) -> Option<(Range<i32>, &Block<Id>)>
    where
        Id: Eq,
    {
        self.iter().find(|(_, block)| block.id == *id)
    }

    pub fn push_top(&mut self, block: Block<Id>) {
        assert!(!self.end.top);
        self.range.top -= block.height();
        self.blocks.push_front(block);
    }

    pub fn push_bottom(&mut self, block: Block<Id>) {
        assert!(!self.end.bottom);
        self.range.bottom += block.height();
        self.blocks.push_back(block);
    }

    pub fn append_top(&mut self, other: Self) {
        assert!(!self.end.top);
        assert!(!other.end.bottom);
        for block in other.blocks.into_iter().rev() {
            self.push_top(block);
        }
        self.end.top = other.end.top;
    }

    pub fn append_bottom(&mut self, other: Self) {
        assert!(!self.end.bottom);
        assert!(!other.end.top);
        for block in other.blocks {
            self.push_bottom(block);
        }
        self.end.bottom = other.end.bottom;
    }

    pub fn end_top(&mut self) {
        self.end.top = true;
    }

    pub fn end_bottom(&mut self) {
        self.end.bottom = true;
    }

    pub fn shift(&mut self, delta: i32) {
        self.range = self.range.shifted(delta);
    }

    pub fn set_top(&mut self, top: i32) {
        self.shift(top - self.range.top);
    }

    pub fn set_bottom(&mut self, bottom: i32) {
        self.shift(bottom - self.range.bottom);
    }
}

pub struct Iter<'a, Id> {
    iter: vec_deque::Iter<'a, Block<Id>>,
    range: Range<i32>,
}

impl<'a, Id> Iterator for Iter<'a, Id> {
    type Item = (Range<i32>, &'a Block<Id>);

    fn next(&mut self) -> Option<Self::Item> {
        let block = self.iter.next()?;
        let range = Range::new(self.range.top, self.range.top + block.height());
        self.range.top = range.bottom;
        Some((range, block))
    }
}

impl<Id> DoubleEndedIterator for Iter<'_, Id> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let block = self.iter.next_back()?;
        let range = Range::new(self.range.bottom - block.height(), self.range.bottom);
        self.range.bottom = range.top;
        Some((range, block))
    }
}

pub struct IntoIter<Id> {
    iter: vec_deque::IntoIter<Block<Id>>,
    range: Range<i32>,
}

impl<Id> Iterator for IntoIter<Id> {
    type Item = (Range<i32>, Block<Id>);

    fn next(&mut self) -> Option<Self::Item> {
        let block = self.iter.next()?;
        let range = Range::new(self.range.top, self.range.top + block.height());
        self.range.top = range.bottom;
        Some((range, block))
    }
}

impl<Id> DoubleEndedIterator for IntoIter<Id> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let block = self.iter.next_back()?;
        let range = Range::new(self.range.bottom - block.height(), self.range.bottom);
        self.range.bottom = range.top;
        Some((range, block))
    }
}
