//! Rendering messages as full trees.

// TODO Focusing on sub-trees

mod renderer;
mod scroll;
mod widgets;

use std::collections::HashSet;

use async_trait::async_trait;
use cove_config::Keys;
use cove_input::InputEvent;
use jiff::tz::TimeZone;
use toss::widgets::EditorState;
use toss::{AsyncWidget, Frame, Pos, Size, WidgetExt, WidthDb};

use crate::store::{Msg, MsgStore};
use crate::ui::{ChatMsg, UiError, util};
use crate::util::InfallibleExt;

use self::renderer::{TreeContext, TreeRenderer};

use super::Reaction;
use super::cursor::Cursor;

pub struct TreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,
    tz: TimeZone,

    last_size: Size,
    last_nick: String,
    last_cursor: Cursor<M::Id>,
    last_cursor_top: i32,
    last_visible_msgs: Vec<M::Id>,

    folded: HashSet<M::Id>,
}

impl<M: Msg, S: MsgStore<M>> TreeViewState<M, S> {
    pub fn new(store: S, tz: TimeZone) -> Self {
        Self {
            store,
            tz,
            last_size: Size::ZERO,
            last_nick: String::new(),
            last_cursor: Cursor::Bottom,
            last_cursor_top: 0,
            last_visible_msgs: vec![],
            folded: HashSet::new(),
        }
    }

    async fn handle_movement_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
    ) -> Result<bool, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        let chat_height: i32 = (event.frame().size().height - 3).into();

        // Basic cursor movement
        if event.matches(&keys.cursor.up) {
            cursor.move_up_in_tree(&self.store, &self.folded).await?;
            return Ok(true);
        }
        if event.matches(&keys.cursor.down) {
            cursor.move_down_in_tree(&self.store, &self.folded).await?;
            return Ok(true);
        }
        if event.matches(&keys.cursor.to_top) {
            cursor.move_to_top(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.cursor.to_bottom) {
            cursor.move_to_bottom();
            return Ok(true);
        }

        // Tree cursor movement
        if event.matches(&keys.tree.cursor.to_above_sibling) {
            cursor.move_to_prev_sibling(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_below_sibling) {
            cursor.move_to_next_sibling(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_parent) {
            cursor.move_to_parent(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_root) {
            cursor.move_to_root(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_older_message) {
            cursor.move_to_older_msg(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_newer_message) {
            cursor.move_to_newer_msg(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_older_unseen_message) {
            cursor.move_to_older_unseen_msg(&self.store).await?;
            return Ok(true);
        }
        if event.matches(&keys.tree.cursor.to_newer_unseen_message) {
            cursor.move_to_newer_unseen_msg(&self.store).await?;
            return Ok(true);
        }

        // Scrolling
        if event.matches(&keys.scroll.up_line) {
            self.scroll_by(cursor, editor, event.widthdb(), 1).await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.down_line) {
            self.scroll_by(cursor, editor, event.widthdb(), -1).await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.up_half) {
            let delta = chat_height / 2;
            self.scroll_by(cursor, editor, event.widthdb(), delta)
                .await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.down_half) {
            let delta = -(chat_height / 2);
            self.scroll_by(cursor, editor, event.widthdb(), delta)
                .await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.up_full) {
            let delta = chat_height.saturating_sub(1);
            self.scroll_by(cursor, editor, event.widthdb(), delta)
                .await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.down_full) {
            let delta = -chat_height.saturating_sub(1);
            self.scroll_by(cursor, editor, event.widthdb(), delta)
                .await?;
            return Ok(true);
        }
        if event.matches(&keys.scroll.center_cursor) {
            self.center_cursor(cursor, editor, event.widthdb()).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn handle_action_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        id: Option<&M::Id>,
    ) -> Result<bool, S::Error> {
        if event.matches(&keys.tree.action.fold_tree) {
            if let Some(id) = id {
                if !self.folded.remove(id) {
                    self.folded.insert(id.clone());
                }
            }
            return Ok(true);
        }

        if event.matches(&keys.tree.action.toggle_seen) {
            if let Some(id) = id {
                if let Some(msg) = self.store.tree(id).await?.msg(id) {
                    self.store.set_seen(id, !msg.seen()).await?;
                }
            }
            return Ok(true);
        }

        if event.matches(&keys.tree.action.mark_visible_seen) {
            for id in &self.last_visible_msgs {
                self.store.set_seen(id, true).await?;
            }
            return Ok(true);
        }

        if event.matches(&keys.tree.action.mark_older_seen) {
            if let Some(id) = id {
                self.store.set_older_seen(id, true).await?;
            } else {
                self.store
                    .set_older_seen(&M::last_possible_id(), true)
                    .await?;
            }
            return Ok(true);
        }

        Ok(false)
    }

    async fn handle_edit_initiating_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        cursor: &mut Cursor<M::Id>,
        id: Option<M::Id>,
    ) -> Result<bool, S::Error> {
        if event.matches(&keys.tree.action.reply) {
            if let Some(parent) = cursor.parent_for_normal_tree_reply(&self.store).await? {
                *cursor = Cursor::Editor {
                    coming_from: id,
                    parent,
                };
            }
            return Ok(true);
        }

        if event.matches(&keys.tree.action.reply_alternate) {
            if let Some(parent) = cursor.parent_for_alternate_tree_reply(&self.store).await? {
                *cursor = Cursor::Editor {
                    coming_from: id,
                    parent,
                };
            }
            return Ok(true);
        }

        if event.matches(&keys.tree.action.new_thread) {
            *cursor = Cursor::Editor {
                coming_from: id,
                parent: None,
            };
            return Ok(true);
        }

        Ok(false)
    }

    async fn handle_normal_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
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
        if self
            .handle_movement_input_event(event, keys, cursor, editor)
            .await?
        {
            return Ok(true);
        }

        if self
            .handle_action_input_event(event, keys, id.as_ref())
            .await?
        {
            return Ok(true);
        }

        if can_compose
            && self
                .handle_edit_initiating_input_event(event, keys, cursor, id)
                .await?
        {
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_editor_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        coming_from: Option<M::Id>,
        parent: Option<M::Id>,
    ) -> Reaction<M> {
        // Abort edit
        if event.matches(&keys.general.abort) {
            *cursor = coming_from.map(Cursor::Msg).unwrap_or(Cursor::Bottom);
            return Reaction::Handled;
        }

        // Send message
        if event.matches(&keys.general.confirm) {
            let content = editor.text().to_string();
            if content.trim().is_empty() {
                return Reaction::Handled;
            }
            *cursor = Cursor::Pseudo {
                coming_from,
                parent: parent.clone(),
            };
            return Reaction::Composed { parent, content };
        }

        // TODO Tab-completion

        // Editing
        if util::handle_editor_input_event(editor, event, keys, |_| true) {
            return Reaction::Handled;
        }

        Reaction::NotHandled
    }

    pub async fn handle_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
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
                    .handle_normal_input_event(event, keys, cursor, editor, can_compose, None)
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
                    .handle_normal_input_event(event, keys, cursor, editor, can_compose, Some(id))
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
                self.handle_editor_input_event(event, keys, cursor, editor, coming_from, parent)
            }
            Cursor::Pseudo { .. } => {
                if self
                    .handle_movement_input_event(event, keys, cursor, editor)
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
        caesar: i8,
    ) -> TreeView<'a, M, S> {
        TreeView {
            state: self,
            cursor,
            editor,
            nick,
            focused,
            caesar,
        }
    }
}

pub struct TreeView<'a, M: Msg, S: MsgStore<M>> {
    state: &'a mut TreeViewState<M, S>,

    cursor: &'a mut Cursor<M::Id>,
    editor: &'a mut EditorState,

    nick: String,
    focused: bool,
    caesar: i8,
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
            caesar: self.caesar,
            last_cursor: self.state.last_cursor.clone(),
            last_cursor_top: self.state.last_cursor_top,
        };

        let mut renderer = TreeRenderer::new(
            context,
            &self.state.store,
            &self.state.tz,
            &mut self.state.folded,
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
            widget.desync().draw(frame).await.infallible();
            frame.pop();
        }

        Ok(())
    }
}
