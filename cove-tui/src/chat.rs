mod tree;

use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::traits::{Msg, MsgStore};

use self::tree::TreeView;

pub enum Mode {
    Tree,
    // Thread,
    // Flat,
}

pub struct Chat<M: Msg, S: MsgStore<M>> {
    store: S,
    cursor: Option<M::Id>,
    mode: Mode,
    tree: TreeView,
    // thread: ThreadView,
    // flat: FlatView,
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            cursor: None,
            mode: Mode::Tree,
            tree: TreeView::new(),
        }
    }
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn handle_key_event(&mut self, event: KeyEvent, size: Size) {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_key_event(&mut self.store, &mut self.cursor, event, size)
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size) {
        match self.mode {
            Mode::Tree => self.tree.render(&mut self.store, frame, pos, size),
        }
    }
}
