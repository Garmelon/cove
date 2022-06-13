mod tree;

use crossterm::event::KeyEvent;
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore};

use self::tree::TreeView;

pub enum Mode {
    Tree,
    // Thread,
    // Flat,
}

pub struct Cursor<I> {
    id: I,
    /// Where on the screen the cursor is visible (`0.0` = first line, `1.0` =
    /// last line).
    proportion: f32,
}

pub struct Chat<M: Msg, S: MsgStore<M>> {
    store: S,
    room: String,
    cursor: Option<Cursor<M::Id>>,
    mode: Mode,
    tree: TreeView<M>,
    // thread: ThreadView,
    // flat: FlatView,
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn new(store: S, room: String) -> Self {
        Self {
            store,
            room,
            cursor: None,
            mode: Mode::Tree,
            tree: TreeView::new(),
        }
    }
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub fn handle_key_event(&mut self, event: KeyEvent, size: Size) {
        match self.mode {
            Mode::Tree => self.tree.handle_key_event(
                &mut self.store,
                &self.room,
                &mut self.cursor,
                event,
                size,
            ),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size) {
        match self.mode {
            Mode::Tree => self
                .tree
                .render(&mut self.store, &self.room, frame, pos, size),
        }
    }
}
