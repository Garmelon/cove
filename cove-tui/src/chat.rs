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

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    pub async fn handle_navigation(
        &mut self,
        terminal: &mut Terminal,
        size: Size,
        event: KeyEvent,
    ) {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_navigation(&mut self.store, &mut self.cursor, terminal, size, event)
                    .await
            }
        }
    }

    pub async fn handle_messaging(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> Option<(Option<M::Id>, String)> {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_messaging(
                        &mut self.store,
                        &mut self.cursor,
                        terminal,
                        crossterm_lock,
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
