mod tree;

use std::sync::Arc;

use crossterm::event::KeyEvent;
use parking_lot::FairMutex;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

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

impl<I> Cursor<I> {
    /// Create a new cursor with arbitrary proportion.
    pub fn new(id: I) -> Self {
        Self {
            id,
            proportion: 0.0,
        }
    }
}

pub struct Chat<M: Msg, S: MsgStore<M>> {
    store: S,
    cursor: Option<Cursor<M::Id>>,
    mode: Mode,
    tree: TreeView<M>,
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

pub enum Handled<I> {
    Ok,
    NewMessage { parent: Option<I>, content: String },
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub async fn handle_key_event(
        &mut self,
        event: KeyEvent,
        terminal: &mut Terminal,
        size: Size,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> Handled<M::Id> {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_key_event(
                        crossterm_lock,
                        &mut self.store,
                        &mut self.cursor,
                        terminal,
                        size,
                        event,
                    )
                    .await
            }
        }
    }

    pub async fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size) {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .render(&mut self.store, &self.cursor, frame, pos, size)
                    .await
            }
        }
    }
}
