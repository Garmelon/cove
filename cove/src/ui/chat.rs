use cove_config::Keys;
use cove_input::InputEvent;
use jiff::{Timestamp, tz::TimeZone};
use toss::{
    Styled, WidgetExt,
    widgets::{BoxedAsync, EditorState},
};

use crate::{
    store::{Msg, MsgStore},
    util,
};

use super::UiError;

use self::{cursor::Cursor, tree::TreeViewState};

mod blocks;
mod cursor;
mod renderer;
mod tree;
mod widgets;

pub trait ChatMsg {
    fn time(&self) -> Option<Timestamp>;
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
    nick_emoji: bool,
    caesar: i8,

    mode: Mode,
    tree: TreeViewState<M, S>,
}

impl<M: Msg, S: MsgStore<M> + Clone> ChatState<M, S> {
    pub fn new(store: S, tz: TimeZone) -> Self {
        Self {
            cursor: Cursor::Bottom,
            editor: EditorState::new(),
            nick_emoji: false,
            caesar: 0,

            mode: Mode::Tree,
            tree: TreeViewState::new(store.clone(), tz),

            store,
        }
    }

    pub fn nick_emoji(&self) -> bool {
        self.nick_emoji
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
                .widget(
                    &mut self.cursor,
                    &mut self.editor,
                    nick,
                    focused,
                    self.nick_emoji,
                    self.caesar,
                )
                .boxed_async(),
        }
    }

    pub async fn handle_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
        can_compose: bool,
    ) -> Result<Reaction<M>, S::Error>
    where
        M: ChatMsg + Send + Sync,
        M::Id: Send + Sync,
        S: Send + Sync,
        S::Error: Send,
    {
        let reaction = match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_input_event(
                        event,
                        keys,
                        &mut self.cursor,
                        &mut self.editor,
                        can_compose,
                    )
                    .await?
            }
        };

        Ok(match reaction {
            Reaction::Composed { parent, content } if self.caesar != 0 => {
                let content = util::caesar(&content, self.caesar);
                Reaction::Composed { parent, content }
            }

            Reaction::NotHandled if event.matches(&keys.tree.action.toggle_nick_emoji) => {
                self.nick_emoji = !self.nick_emoji;
                Reaction::Handled
            }

            Reaction::NotHandled if event.matches(&keys.tree.action.increase_caesar) => {
                self.caesar = (self.caesar + 1).rem_euclid(26);
                Reaction::Handled
            }

            Reaction::NotHandled if event.matches(&keys.tree.action.decrease_caesar) => {
                self.caesar = (self.caesar - 1).rem_euclid(26);
                Reaction::Handled
            }

            reaction => reaction,
        })
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
            self.tree.send_successful(&id);
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
}

impl<M: Msg> Reaction<M> {
    pub fn handled(&self) -> bool {
        !matches!(self, Self::NotHandled)
    }
}
