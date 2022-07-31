use crate::ui::chat::blocks::Blocks;

use super::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockId<I> {
    Msg(I),
    Cursor,
    LastCursor,
}

impl<I: Clone> BlockId<I> {
    pub fn from_cursor(cursor: &Cursor<I>) -> Self {
        match cursor {
            Cursor::Msg(id) => Self::Msg(id.clone()),
            _ => Self::Cursor,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Root<I> {
    Bottom,
    Tree(I),
}

pub struct TreeBlocks<I> {
    blocks: Blocks<BlockId<I>>,
    top_root: Root<I>,
    bottom_root: Root<I>,
}

impl<I> TreeBlocks<I> {
    pub fn new(top_root: Root<I>, bottom_root: Root<I>) -> Self {
        Self {
            blocks: Blocks::new(),
            top_root,
            bottom_root,
        }
    }

    pub fn blocks(&self) -> &Blocks<BlockId<I>> {
        &self.blocks
    }

    pub fn blocks_mut(&mut self) -> &mut Blocks<BlockId<I>> {
        &mut self.blocks
    }

    pub fn into_blocks(self) -> Blocks<BlockId<I>> {
        self.blocks
    }

    pub fn top_root(&self) -> &Root<I> {
        &self.top_root
    }

    pub fn bottom_root(&self) -> &Root<I> {
        &self.bottom_root
    }

    pub fn prepend(&mut self, other: Self) {
        self.blocks.prepend(other.blocks);
        self.top_root = other.top_root;
    }

    pub fn append(&mut self, other: Self) {
        self.blocks.append(other.blocks);
        self.bottom_root = other.bottom_root;
    }
}
