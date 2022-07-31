mod time;

use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::KeyEvent;
use parking_lot::FairMutex;
use tokio::sync::Mutex;
use toss::frame::{Frame, Size};
use toss::terminal::Terminal;

use crate::store::{Msg, MsgStore};
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::Widget;

///////////
// State //
///////////

pub enum Cursor<I> {
    Bottom,
    Msg(I),
    Editor(Option<I>),
    Pseudo(Option<I>),
}

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,

    last_cursor: Cursor<M::Id>,
    last_cursor_line: i32,

    cursor: Cursor<M::Id>,
    /// Set to true if the chat should be scrolled such that the cursor is fully
    /// visible (if possible). If set to false, then the cursor itself is moved
    /// to a different message such that it remains visible.
    make_cursor_visible: bool,

    editor: EditorState,
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            last_cursor: Cursor::Bottom,
            last_cursor_line: 0,
            cursor: Cursor::Bottom,
            make_cursor_visible: false,
            editor: EditorState::new(),
        }
    }

    async fn handle_navigation(&mut self, event: KeyEvent) -> bool {
        false
    }

    async fn handle_messaging(
        &self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> Option<(Option<M::Id>, String)> {
        None
    }
}

pub struct TreeViewState<M: Msg, S: MsgStore<M>>(Arc<Mutex<InnerTreeViewState<M, S>>>);

impl<M: Msg, S: MsgStore<M>> TreeViewState<M, S> {
    pub fn new(store: S) -> Self {
        Self(Arc::new(Mutex::new(InnerTreeViewState::new(store))))
    }

    pub fn widget(&self) -> TreeView<M, S> {
        TreeView(self.0.clone())
    }

    pub async fn handle_navigation(&mut self, event: KeyEvent) -> bool {
        self.0.lock().await.handle_navigation(event).await
    }

    pub async fn handle_messaging(
        &self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> Option<(Option<M::Id>, String)> {
        self.0
            .lock()
            .await
            .handle_messaging(terminal, crossterm_lock, event)
            .await
    }
}

////////////
// Widget //
////////////

pub struct TreeView<M: Msg, S: MsgStore<M>>(Arc<Mutex<InnerTreeViewState<M, S>>>);

#[async_trait]
impl<M, S> Widget for TreeView<M, S>
where
    M: Msg,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
{
    fn size(&self, _frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        Size::ZERO
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let mut guard = self.0.lock().await;
    }
}
