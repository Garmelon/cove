mod tree;

use async_trait::async_trait;
use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore};

use self::tree::{TreeView, TreeViewState};

use super::widgets::Widget;

///////////
// State //
///////////

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

pub struct ChatState<M: Msg, S: MsgStore<M>> {
    store: S,
    mode: Mode,
    tree: TreeViewState<M, S>,
    // thread: ThreadView,
    // flat: FlatView,
}

impl<M: Msg, S: MsgStore<M> + Clone> ChatState<M, S> {
    pub fn new(store: S) -> Self {
        Self {
            mode: Mode::Tree,
            tree: TreeViewState::new(store.clone()),
            store,
        }
    }
}

impl<M: Msg, S: MsgStore<M>> ChatState<M, S> {
    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn widget(&self) -> Chat<M, S> {
        match self.mode {
            Mode::Tree => Chat::Tree(self.tree.widget()),
        }
    }
}

impl<M: Msg, S: MsgStore<M>> Chat<M, S> {
    // pub async fn handle_navigation(
    //     &mut self,
    //     terminal: &mut Terminal,
    //     size: Size,
    //     event: KeyEvent,
    // ) {
    //     match self.mode {
    //         Mode::Tree => {
    //             self.tree
    //                 .handle_navigation(&mut self.store, &mut self.cursor, terminal, size, event)
    //                 .await
    //         }
    //     }
    // }

    // pub async fn handle_messaging(
    //     &mut self,
    //     terminal: &mut Terminal,
    //     crossterm_lock: &Arc<FairMutex<()>>,
    //     event: KeyEvent,
    // ) -> Option<(Option<M::Id>, String)> {
    //     match self.mode {
    //         Mode::Tree => {
    //             self.tree
    //                 .handle_messaging(
    //                     &mut self.store,
    //                     &mut self.cursor,
    //                     terminal,
    //                     crossterm_lock,
    //                     event,
    //                 )
    //                 .await
    //         }
    //     }
    // }

    // pub async fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size) {
    //     match self.mode {
    //         Mode::Tree => {
    //             self.tree
    //                 .render(&mut self.store, &self.cursor, frame, pos, size)
    //                 .await
    //         }
    //     }
    // }
}

////////////
// Widget //
////////////

pub enum Chat<M: Msg, S: MsgStore<M>> {
    Tree(TreeView<M, S>),
}

#[async_trait]
impl<M, S> Widget for Chat<M, S>
where
    M: Msg,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
{
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        match self {
            Self::Tree(tree) => tree.size(frame, max_width, max_height),
        }
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        match *self {
            Self::Tree(tree) => Box::new(tree).render(frame).await,
        }
    }
}
