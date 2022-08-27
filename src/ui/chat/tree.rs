mod cursor;
mod layout;
mod tree_blocks;
mod widgets;

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::KeyCode;
use parking_lot::FairMutex;
use tokio::sync::Mutex;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

use crate::store::{Msg, MsgStore};
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::util;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::Widget;

use self::cursor::Cursor;

use super::{ChatMsg, Reaction};

///////////
// State //
///////////

enum Correction {
    MakeCursorVisible,
    MoveCursorToVisibleArea,
    CenterCursor,
}

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,

    last_cursor: Cursor<M::Id>,
    last_cursor_line: i32,
    last_visible_msgs: Vec<M::Id>,

    cursor: Cursor<M::Id>,
    editor: EditorState,

    /// Scroll the view on the next render. Positive values scroll up and
    /// negative values scroll down.
    scroll: i32,
    correction: Option<Correction>,

    folded: HashSet<M::Id>,
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            last_cursor: Cursor::Bottom,
            last_cursor_line: 0,
            last_visible_msgs: vec![],
            cursor: Cursor::Bottom,
            editor: EditorState::new(),
            scroll: 0,
            correction: None,
            folded: HashSet::new(),
        }
    }

    pub fn list_movement_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("j/k, ↓/↑", "move cursor up/down");
        bindings.binding("J/K, ctrl+↓/↑", "move cursor to prev/next sibling");
        bindings.binding("h/l, ←/→", "move cursor chronologically");
        bindings.binding("H/L, ctrl+←/→", "move cursor to prev/next unseen message");
        bindings.binding("g, home", "move cursor to top");
        bindings.binding("G, end", "move cursor to bottom");
        bindings.binding("p/P", "move cursor to parent/top-level parent");
        bindings.binding("ctrl+y/e", "scroll up/down a line");
        bindings.binding("ctrl+u/d", "scroll up/down half a screen");
        bindings.binding("ctrl+b/f, page up/down", "scroll up/down one screen");
        bindings.binding("z", "center cursor on screen");
    }

    async fn handle_movement_input_event(&mut self, frame: &mut Frame, event: &InputEvent) -> bool {
        let chat_height = frame.size().height - 3;

        match event {
            key!('k') | key!(Up) => self.move_cursor_up().await,
            key!('j') | key!(Down) => self.move_cursor_down().await,
            key!('K') | key!(Ctrl + Up) => self.move_cursor_up_sibling().await,
            key!('J') | key!(Ctrl + Down) => self.move_cursor_down_sibling().await,
            key!('h') | key!(Left) => self.move_cursor_older().await,
            key!('l') | key!(Right) => self.move_cursor_newer().await,
            key!('H') | key!(Ctrl + Left) => self.move_cursor_older_unseen().await,
            key!('L') | key!(Ctrl + Right) => self.move_cursor_newer_unseen().await,
            key!('g') | key!(Home) => self.move_cursor_to_top().await,
            key!('G') | key!(End) => self.move_cursor_to_bottom().await,
            key!('p') => self.move_cursor_to_parent().await,
            key!('P') => self.move_cursor_to_root().await,
            key!(Ctrl + 'y') => self.scroll_up(1),
            key!(Ctrl + 'e') => self.scroll_down(1),
            key!(Ctrl + 'u') => self.scroll_up((chat_height / 2).into()),
            key!(Ctrl + 'd') => self.scroll_down((chat_height / 2).into()),
            key!(Ctrl + 'b') | key!(PageUp) => self.scroll_up(chat_height.saturating_sub(1).into()),
            key!(Ctrl + 'f') | key!(PageDown) => {
                self.scroll_down(chat_height.saturating_sub(1).into())
            }
            key!('z') => self.center_cursor(),
            _ => return false,
        }

        true
    }

    pub fn list_action_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("space", "fold current message's subtree");
        bindings.binding("s", "toggle current message's seen status");
        bindings.binding("S", "mark all visible messages as seen");
        bindings.binding("ctrl+s", "mark all older messages as seen");
    }

    async fn handle_action_input_event(&mut self, event: &InputEvent, id: Option<&M::Id>) -> bool {
        match event {
            key!(' ') => {
                if let Some(id) = id {
                    if !self.folded.remove(id) {
                        self.folded.insert(id.clone());
                    }
                    return true;
                }
            }
            key!('s') => {
                if let Some(id) = id {
                    if let Some(msg) = self.store.tree(id).await.msg(id) {
                        self.store.set_seen(id, !msg.seen()).await;
                    }
                    return true;
                }
            }
            key!('S') => {
                for id in &self.last_visible_msgs {
                    self.store.set_seen(id, true).await;
                }
                return true;
            }
            key!(Ctrl + 's') => {
                if let Some(id) = id {
                    self.store.set_older_seen(id, true).await;
                } else {
                    self.store
                        .set_older_seen(&M::last_possible_id(), true)
                        .await;
                }
                return true;
            }
            _ => {}
        }
        false
    }

    pub fn list_edit_initiating_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.empty();
        bindings.binding("r", "reply to message");
        bindings.binding_ctd("(inline if possible, otherwise directly)");
        bindings.binding("R", "reply to message (opposite of R)");
        bindings.binding("t", "start a new thread");
    }

    async fn handle_edit_initiating_input_event(
        &mut self,
        event: &InputEvent,
        id: Option<M::Id>,
    ) -> bool {
        match event {
            key!('r') => {
                if let Some(parent) = self.parent_for_normal_reply().await {
                    self.cursor = Cursor::editor(id, parent);
                    self.correction = Some(Correction::MakeCursorVisible);
                }
            }
            key!('R') => {
                if let Some(parent) = self.parent_for_alternate_reply().await {
                    self.cursor = Cursor::editor(id, parent);
                    self.correction = Some(Correction::MakeCursorVisible);
                }
            }
            key!('t') | key!('T') => {
                self.cursor = Cursor::editor(id, None);
                self.correction = Some(Correction::MakeCursorVisible);
            }
            _ => return false,
        }

        true
    }

    pub fn list_normal_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        self.list_movement_key_bindings(bindings);
        self.list_action_key_bindings(bindings);
        if can_compose {
            self.list_edit_initiating_key_bindings(bindings);
        }
    }

    async fn handle_normal_input_event(
        &mut self,
        frame: &mut Frame,
        event: &InputEvent,
        can_compose: bool,
        id: Option<M::Id>,
    ) -> bool {
        #[allow(clippy::if_same_then_else)]
        if self.handle_movement_input_event(frame, event).await {
            true
        } else if self.handle_action_input_event(event, id.as_ref()).await {
            true
        } else if can_compose {
            self.handle_edit_initiating_input_event(event, id).await
        } else {
            false
        }
    }

    fn list_editor_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close editor");
        bindings.binding("enter", "send message");
        util::list_editor_key_bindings(bindings, |_| true, true);
    }

    fn handle_editor_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        coming_from: Option<M::Id>,
        parent: Option<M::Id>,
    ) -> Reaction<M> {
        // TODO Tab-completion
        match event {
            key!(Esc) => {
                self.cursor = coming_from.map(Cursor::Msg).unwrap_or(Cursor::Bottom);
                self.correction = Some(Correction::MakeCursorVisible);
                return Reaction::Handled;
            }

            key!(Enter) => {
                let content = self.editor.text();
                if !content.trim().is_empty() {
                    self.cursor = Cursor::Pseudo {
                        coming_from,
                        parent: parent.clone(),
                    };
                    return Reaction::Composed { parent, content };
                }
            }

            _ => {
                let handled = util::handle_editor_input_event(
                    &self.editor,
                    terminal,
                    crossterm_lock,
                    event,
                    |_| true,
                    true,
                );
                if !handled {
                    return Reaction::NotHandled;
                }
            }
        }

        self.correction = Some(Correction::MakeCursorVisible);
        Reaction::Handled
    }

    pub fn list_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        bindings.heading("Chat");
        match &self.cursor {
            Cursor::Bottom | Cursor::Msg(_) => {
                self.list_normal_key_bindings(bindings, can_compose);
            }
            Cursor::Editor { .. } => self.list_editor_key_bindings(bindings),
            Cursor::Pseudo { .. } => {
                self.list_normal_key_bindings(bindings, false);
            }
        }
    }

    async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        can_compose: bool,
    ) -> Reaction<M> {
        match &self.cursor {
            Cursor::Bottom => {
                if self
                    .handle_normal_input_event(terminal.frame(), event, can_compose, None)
                    .await
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
            Cursor::Msg(id) => {
                let id = id.clone();
                if self
                    .handle_normal_input_event(terminal.frame(), event, can_compose, Some(id))
                    .await
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
            Cursor::Editor {
                coming_from,
                parent,
            } => self.handle_editor_input_event(
                terminal,
                crossterm_lock,
                event,
                coming_from.clone(),
                parent.clone(),
            ),
            Cursor::Pseudo { .. } => {
                if self
                    .handle_movement_input_event(terminal.frame(), event)
                    .await
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
        }
    }

    fn sent(&mut self, id: Option<M::Id>) {
        if let Cursor::Pseudo { coming_from, .. } = &self.cursor {
            if let Some(id) = id {
                self.last_cursor = Cursor::Msg(id.clone());
                self.cursor = Cursor::Msg(id);
                self.editor.clear();
            } else {
                self.cursor = match coming_from {
                    Some(id) => Cursor::Msg(id.clone()),
                    None => Cursor::Bottom,
                };
            };
        }
    }
}

pub struct TreeViewState<M: Msg, S: MsgStore<M>>(Arc<Mutex<InnerTreeViewState<M, S>>>);

impl<M: Msg, S: MsgStore<M>> TreeViewState<M, S> {
    pub fn new(store: S) -> Self {
        Self(Arc::new(Mutex::new(InnerTreeViewState::new(store))))
    }

    pub fn widget(&self, nick: String) -> TreeView<M, S> {
        TreeView {
            inner: self.0.clone(),
            nick,
        }
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        self.0.lock().await.list_key_bindings(bindings, can_compose);
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        can_compose: bool,
    ) -> Reaction<M> {
        self.0
            .lock()
            .await
            .handle_input_event(terminal, crossterm_lock, event, can_compose)
            .await
    }

    pub async fn sent(&mut self, id: Option<M::Id>) {
        self.0.lock().await.sent(id)
    }
}

////////////
// Widget //
////////////

pub struct TreeView<M: Msg, S: MsgStore<M>> {
    inner: Arc<Mutex<InnerTreeViewState<M, S>>>,
    nick: String,
}

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
        let mut guard = self.inner.lock().await;
        let blocks = guard.relayout(&self.nick, frame).await;

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
