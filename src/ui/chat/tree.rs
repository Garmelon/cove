mod cursor;
mod layout;
mod tree_blocks;
mod widgets;

use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::FairMutex;
use tokio::sync::Mutex;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

use crate::store::{Msg, MsgStore};
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::Widget;

use self::cursor::Cursor;

use super::ChatMsg;

///////////
// State //
///////////

enum Correction {
    MakeCursorVisible,
    MoveCursorToVisibleArea,
}

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,

    last_cursor: Cursor<M::Id>,
    last_cursor_line: i32,

    cursor: Cursor<M::Id>,

    /// Scroll the view on the next render. Positive values scroll up and
    /// negative values scroll down.
    scroll: i32,
    correction: Option<Correction>,

    editor: EditorState,
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            last_cursor: Cursor::Bottom,
            last_cursor_line: 0,
            cursor: Cursor::Bottom,
            scroll: 0,
            correction: None,
            editor: EditorState::new(),
        }
    }

    async fn handle_navigation(&mut self, event: KeyEvent) -> bool {
        match event.code {
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor_up().await,
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor_down().await,
            KeyCode::Char('g') | KeyCode::Home => self.move_cursor_to_top().await,
            KeyCode::Char('G') | KeyCode::End => self.move_cursor_to_bottom().await,
            KeyCode::Char('y') if event.modifiers == KeyModifiers::CONTROL => self.scroll_up(1),
            KeyCode::Char('e') if event.modifiers == KeyModifiers::CONTROL => self.scroll_down(1),
            _ => return false,
        }
        true
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
    M: Msg + ChatMsg,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
{
    fn size(&self, _frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        Size::ZERO
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let mut guard = self.0.lock().await;
        let blocks = guard.relayout(frame).await;

        let size = frame.size();
        for block in blocks.into_blocks().blocks {
            frame.push(
                Pos::new(0, block.top_line),
                Size::new(size.width, block.height as u16),
            );
            block.widget.render(frame).await;
            frame.pop();
        }
    }
}
