mod blocks;
mod cursor;
mod renderer;
mod tree;
mod widgets;

use std::io;
use std::sync::Arc;

use parking_lot::FairMutex;
use time::OffsetDateTime;
use toss::widgets::{BoxedAsync, EditorState};
use toss::{Styled, Terminal, WidgetExt};

use crate::store::{Msg, MsgStore};

use self::cursor::Cursor;
use self::tree::TreeViewState;

use super::input::{InputEvent, KeyBindingsList};
use super::UiError;

pub trait ChatMsg {
    fn time(&self) -> OffsetDateTime;
    fn styled(&self) -> (Styled, Styled);
    fn edit(nick: &str, content: &str) -> (Styled, Styled);
    fn pseudo(nick: &str, content: &str) -> (Styled, Styled);
}

pub enum Mode {
    Tree,
}

pub struct ChatState<M: Msg, S: MsgStore<M>> {
    store: S,

    cursor: Cursor<M::Id>,
    editor: EditorState,

    mode: Mode,
    tree: TreeViewState<M, S>,
}

impl<M: Msg, S: MsgStore<M> + Clone> ChatState<M, S> {
    pub fn new(store: S) -> Self {
        Self {
            cursor: Cursor::Bottom,
            editor: EditorState::new(),

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

    pub fn widget(&mut self, nick: String, focused: bool) -> BoxedAsync<'_, UiError>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
        UiError: From<S::Error>,
    {
        match self.mode {
            Mode::Tree => self
                .tree
                .widget(&mut self.cursor, &mut self.editor, nick, focused)
                .boxed_async(),
        }
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        match self.mode {
            Mode::Tree => self
                .tree
                .list_key_bindings(bindings, &self.cursor, can_compose),
        }
        todo!()
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        can_compose: bool,
    ) -> Result<Reaction<M>, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_input_event(
                        terminal,
                        crossterm_lock,
                        event,
                        &mut self.cursor,
                        &mut self.editor,
                        can_compose,
                    )
                    .await
            }
        }
    }

    pub fn cursor(&self) -> Option<&M::Id> {
        match &self.cursor {
            Cursor::Msg(id) => Some(id),
            Cursor::Bottom | Cursor::Editor { .. } | Cursor::Pseudo { .. } => None,
        }
    }

    /// A [`Reaction::Composed`] message was sent successfully.
    pub fn send_successful(&mut self, id: M::Id) {
        if let Cursor::Pseudo { .. } = &self.cursor {
            self.cursor = Cursor::Msg(id);
            self.editor.clear();
        }
    }

    /// A [`Reaction::Composed`] message failed to be sent.
    pub fn send_failed(&mut self) {
        if let Cursor::Pseudo { coming_from, .. } = &self.cursor {
            self.cursor = match coming_from {
                Some(id) => Cursor::Msg(id.clone()),
                None => Cursor::Bottom,
            };
        }
    }
}

pub enum Reaction<M: Msg> {
    NotHandled,
    Handled,
    Composed {
        parent: Option<M::Id>,
        content: String,
    },
    ComposeError(io::Error),
}

impl<M: Msg> Reaction<M> {
    pub fn handled(&self) -> bool {
        !matches!(self, Self::NotHandled)
    }
}
