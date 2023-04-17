//! Rendering messages as full trees.

// TODO Focusing on sub-trees

mod renderer;
mod scroll;
mod widgets;

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::FairMutex;
use toss::widgets::EditorState;
use toss::{AsyncWidget, Frame, Pos, Size, Terminal, WidthDb};

use crate::store::{Msg, MsgStore};
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::{util, ChatMsg, UiError};
use crate::util::InfallibleExt;

use self::renderer::{TreeContext, TreeRenderer};

use super::cursor::Cursor;
use super::Reaction;

pub struct TreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,

    last_size: Size,
    last_nick: String,
    last_cursor: Cursor<M::Id>,
    last_cursor_top: i32,
    last_visible_msgs: Vec<M::Id>,

    folded: HashSet<M::Id>,
}

impl<M: Msg, S: MsgStore<M>> TreeViewState<M, S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            last_size: Size::ZERO,
            last_nick: String::new(),
            last_cursor: Cursor::Bottom,
            last_cursor_top: 0,
            last_visible_msgs: vec![],
            folded: HashSet::new(),
        }
    }

    pub fn list_movement_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("j/k, ↓/↑", "move cursor up/down");
        bindings.binding("J/K, ctrl+↓/↑", "move cursor to prev/next sibling");
        bindings.binding("p/P", "move cursor to parent/root");
        bindings.binding("h/l, ←/→", "move cursor chronologically");
        bindings.binding("H/L, ctrl+←/→", "move cursor to prev/next unseen message");
        bindings.binding("g, home", "move cursor to top");
        bindings.binding("G, end", "move cursor to bottom");
        bindings.binding("ctrl+y/e", "scroll up/down a line");
        bindings.binding("ctrl+u/d", "scroll up/down half a screen");
        bindings.binding("ctrl+b/f, page up/down", "scroll up/down one screen");
        bindings.binding("z", "center cursor on screen");
        // TODO Bindings inspired by vim's ()/[]/{} bindings?
    }

    async fn handle_movement_input_event(
        &mut self,
        frame: &mut Frame,
        event: &InputEvent,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
    ) -> Result<bool, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        let chat_height: i32 = (frame.size().height - 3).into();
        let widthdb = frame.widthdb();

        match event {
            key!('k') | key!(Up) => cursor.move_up_in_tree(&self.store, &self.folded).await?,
            key!('j') | key!(Down) => cursor.move_down_in_tree(&self.store, &self.folded).await?,
            key!('K') | key!(Ctrl + Up) => cursor.move_to_prev_sibling(&self.store).await?,
            key!('J') | key!(Ctrl + Down) => cursor.move_to_next_sibling(&self.store).await?,
            key!('p') => cursor.move_to_parent(&self.store).await?,
            key!('P') => cursor.move_to_root(&self.store).await?,
            key!('h') | key!(Left) => cursor.move_to_older_msg(&self.store).await?,
            key!('l') | key!(Right) => cursor.move_to_newer_msg(&self.store).await?,
            key!('H') | key!(Ctrl + Left) => cursor.move_to_older_unseen_msg(&self.store).await?,
            key!('L') | key!(Ctrl + Right) => cursor.move_to_newer_unseen_msg(&self.store).await?,
            key!('g') | key!(Home) => cursor.move_to_top(&self.store).await?,
            key!('G') | key!(End) => cursor.move_to_bottom(),
            key!(Ctrl + 'y') => self.scroll_by(cursor, editor, widthdb, 1).await?,
            key!(Ctrl + 'e') => self.scroll_by(cursor, editor, widthdb, -1).await?,
            key!(Ctrl + 'u') => {
                let delta = chat_height / 2;
                self.scroll_by(cursor, editor, widthdb, delta).await?;
            }
            key!(Ctrl + 'd') => {
                let delta = -(chat_height / 2);
                self.scroll_by(cursor, editor, widthdb, delta).await?;
            }
            key!(Ctrl + 'b') | key!(PageUp) => {
                let delta = chat_height.saturating_sub(1);
                self.scroll_by(cursor, editor, widthdb, delta).await?;
            }
            key!(Ctrl + 'f') | key!(PageDown) => {
                let delta = -chat_height.saturating_sub(1);
                self.scroll_by(cursor, editor, widthdb, delta).await?;
            }
            key!('z') => self.center_cursor(cursor, editor, widthdb).await?,
            _ => return Ok(false),
        }

        Ok(true)
    }

    pub fn list_action_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("space", "fold current message's subtree");
        bindings.binding("s", "toggle current message's seen status");
        bindings.binding("S", "mark all visible messages as seen");
        bindings.binding("ctrl+s", "mark all older messages as seen");
    }

    async fn handle_action_input_event(
        &mut self,
        event: &InputEvent,
        id: Option<&M::Id>,
    ) -> Result<bool, S::Error> {
        match event {
            key!(' ') => {
                if let Some(id) = id {
                    if !self.folded.remove(id) {
                        self.folded.insert(id.clone());
                    }
                    return Ok(true);
                }
            }
            key!('s') => {
                if let Some(id) = id {
                    if let Some(msg) = self.store.tree(id).await?.msg(id) {
                        self.store.set_seen(id, !msg.seen()).await?;
                    }
                    return Ok(true);
                }
            }
            key!('S') => {
                for id in &self.last_visible_msgs {
                    self.store.set_seen(id, true).await?;
                }
                return Ok(true);
            }
            key!(Ctrl + 's') => {
                if let Some(id) = id {
                    self.store.set_older_seen(id, true).await?;
                } else {
                    self.store
                        .set_older_seen(&M::last_possible_id(), true)
                        .await?;
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    pub fn list_edit_initiating_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("r", "reply to message (inline if possible, else directly)");
        bindings.binding("R", "reply to message (opposite of R)");
        bindings.binding("t", "start a new thread");
    }

    async fn handle_edit_initiating_input_event(
        &mut self,
        event: &InputEvent,
        cursor: &mut Cursor<M::Id>,
        id: Option<M::Id>,
    ) -> Result<bool, S::Error> {
        match event {
            key!('r') => {
                if let Some(parent) = cursor.parent_for_normal_tree_reply(&self.store).await? {
                    *cursor = Cursor::Editor {
                        coming_from: id,
                        parent,
                    };
                }
            }
            key!('R') => {
                if let Some(parent) = cursor.parent_for_alternate_tree_reply(&self.store).await? {
                    *cursor = Cursor::Editor {
                        coming_from: id,
                        parent,
                    };
                }
            }
            key!('t') | key!('T') => {
                *cursor = Cursor::Editor {
                    coming_from: id,
                    parent: None,
                };
            }
            _ => return Ok(false),
        }

        Ok(true)
    }

    pub fn list_normal_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        self.list_movement_key_bindings(bindings);
        bindings.empty();
        self.list_action_key_bindings(bindings);
        if can_compose {
            bindings.empty();
            self.list_edit_initiating_key_bindings(bindings);
        }
    }

    async fn handle_normal_input_event(
        &mut self,
        frame: &mut Frame,
        event: &InputEvent,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        can_compose: bool,
        id: Option<M::Id>,
    ) -> Result<bool, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        #[allow(clippy::if_same_then_else)]
        Ok(
            if self
                .handle_movement_input_event(frame, event, cursor, editor)
                .await?
            {
                true
            } else if self.handle_action_input_event(event, id.as_ref()).await? {
                true
            } else if can_compose {
                self.handle_edit_initiating_input_event(event, cursor, id)
                    .await?
            } else {
                false
            },
        )
    }

    fn list_editor_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close editor");
        bindings.binding("enter", "send message");
        util::list_editor_key_bindings_allowing_external_editing(bindings, |_| true);
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_editor_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        coming_from: Option<M::Id>,
        parent: Option<M::Id>,
    ) -> Reaction<M> {
        // TODO Tab-completion

        match event {
            key!(Esc) => {
                *cursor = coming_from.map(Cursor::Msg).unwrap_or(Cursor::Bottom);
                return Reaction::Handled;
            }

            key!(Enter) => {
                let content = editor.text().to_string();
                if !content.trim().is_empty() {
                    *cursor = Cursor::Pseudo {
                        coming_from,
                        parent: parent.clone(),
                    };
                    return Reaction::Composed { parent, content };
                }
            }

            _ => {
                let handled = util::handle_editor_input_event_allowing_external_editing(
                    editor,
                    terminal,
                    crossterm_lock,
                    event,
                    |_| true,
                );
                match handled {
                    Ok(true) => {}
                    Ok(false) => return Reaction::NotHandled,
                    Err(e) => return Reaction::ComposeError(e),
                }
            }
        }

        Reaction::Handled
    }

    pub fn list_key_bindings(
        &self,
        bindings: &mut KeyBindingsList,
        cursor: &Cursor<M::Id>,
        can_compose: bool,
    ) {
        bindings.heading("Chat");
        match cursor {
            Cursor::Bottom | Cursor::Msg(_) => {
                self.list_normal_key_bindings(bindings, can_compose);
            }
            Cursor::Editor { .. } => self.list_editor_key_bindings(bindings),
            Cursor::Pseudo { .. } => {
                self.list_normal_key_bindings(bindings, false);
            }
        }
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        can_compose: bool,
    ) -> Result<Reaction<M>, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        Ok(match cursor {
            Cursor::Bottom => {
                if self
                    .handle_normal_input_event(
                        terminal.frame(),
                        event,
                        cursor,
                        editor,
                        can_compose,
                        None,
                    )
                    .await?
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
            Cursor::Msg(id) => {
                let id = id.clone();
                if self
                    .handle_normal_input_event(
                        terminal.frame(),
                        event,
                        cursor,
                        editor,
                        can_compose,
                        Some(id),
                    )
                    .await?
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
            Cursor::Editor {
                coming_from,
                parent,
            } => {
                let coming_from = coming_from.clone();
                let parent = parent.clone();
                self.handle_editor_input_event(
                    terminal,
                    crossterm_lock,
                    event,
                    cursor,
                    editor,
                    coming_from,
                    parent,
                )
            }
            Cursor::Pseudo { .. } => {
                if self
                    .handle_movement_input_event(terminal.frame(), event, cursor, editor)
                    .await?
                {
                    Reaction::Handled
                } else {
                    Reaction::NotHandled
                }
            }
        })
    }

    pub fn send_successful(&mut self, id: &M::Id) {
        if let Cursor::Pseudo { .. } = self.last_cursor {
            self.last_cursor = Cursor::Msg(id.clone());
        }
    }

    pub fn widget<'a>(
        &'a mut self,
        cursor: &'a mut Cursor<M::Id>,
        editor: &'a mut EditorState,
        nick: String,
        focused: bool,
    ) -> TreeView<'a, M, S> {
        TreeView {
            state: self,
            cursor,
            editor,
            nick,
            focused,
        }
    }
}

pub struct TreeView<'a, M: Msg, S: MsgStore<M>> {
    state: &'a mut TreeViewState<M, S>,

    cursor: &'a mut Cursor<M::Id>,
    editor: &'a mut EditorState,

    nick: String,
    focused: bool,
}

#[async_trait]
impl<M, S> AsyncWidget<UiError> for TreeView<'_, M, S>
where
    M: Msg + ChatMsg + Send + Sync,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
    S::Error: Send,
    UiError: From<S::Error>,
{
    async fn size(
        &self,
        _widthdb: &mut WidthDb,
        _max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Result<Size, UiError> {
        Ok(Size::ZERO)
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), UiError> {
        let size = frame.size();

        let context = TreeContext {
            size,
            nick: self.nick.clone(),
            focused: self.focused,
            last_cursor: self.state.last_cursor.clone(),
            last_cursor_top: self.state.last_cursor_top,
        };

        let mut renderer = TreeRenderer::new(
            context,
            &self.state.store,
            self.cursor,
            self.editor,
            frame.widthdb(),
        );

        renderer.prepare_blocks_for_drawing().await?;

        self.state.last_size = size;
        self.state.last_nick = self.nick;
        renderer.update_render_info(
            &mut self.state.last_cursor,
            &mut self.state.last_cursor_top,
            &mut self.state.last_visible_msgs,
        );

        for (range, block) in renderer.into_visible_blocks() {
            let widget = block.into_widget();
            frame.push(Pos::new(0, range.top), widget.size());
            widget.draw(frame).await.infallible();
            frame.pop();
        }

        Ok(())
    }
}
