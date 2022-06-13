use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::traits::{Msg, MsgStore};

pub enum Mode {
    Tree,
    // Thread,
    // Flat,
}

pub struct Chat<M: Msg, S: MsgStore<M>> {
    cursor: Option<M::Id>,
    store: S,
    mode: Mode,
    // tree: TreeView,
    // thread: ThreadView,
    // flat: FlatView,
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn new(store: S) -> Self {
        Self {
            cursor: None,
            store,
            mode: Mode::Tree,
        }
    }
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn handle_key_event(&mut self, key: KeyEvent, size: Size) {
        // TODO
    }

    pub fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size) {
        // TODO
        frame.write(Pos::new(0, 0), "Hello world!", ContentStyle::default());
    }
}
