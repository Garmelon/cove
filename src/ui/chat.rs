// TODO Implement thread view
// TODO Implement flat (chronological?) view
// TODO Implement message search?

mod blocks;
mod tree;

use std::sync::Arc;
use std::{fmt, io};

use async_trait::async_trait;
use parking_lot::FairMutex;
use time::OffsetDateTime;
use toss::{Frame, Size, Styled, Terminal, WidthDb};

use crate::store::{Msg, MsgStore};

use self::tree::{TreeView, TreeViewState};

use super::input::{InputEvent, KeyBindingsList};
use super::widgets::Widget;

///////////
// Trait //
///////////

pub trait ChatMsg {
    fn time(&self) -> OffsetDateTime;
    fn styled(&self) -> (Styled, Styled);
    fn edit(nick: &str, content: &str) -> (Styled, Styled);
    fn pseudo(nick: &str, content: &str) -> (Styled, Styled);
}

///////////
// State //
///////////

pub enum Mode {
    Tree,
    // Thread,
    // Flat,
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

    pub fn widget(&self, nick: String, focused: bool) -> Chat<M, S> {
        match self.mode {
            Mode::Tree => Chat::Tree(self.tree.widget(nick, focused)),
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

impl<M: Msg, S: MsgStore<M>> ChatState<M, S> {
    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList, can_compose: bool) {
        match self.mode {
            Mode::Tree => self.tree.list_key_bindings(bindings, can_compose).await,
        }
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        can_compose: bool,
    ) -> Result<Reaction<M>, S::Error> {
        match self.mode {
            Mode::Tree => {
                self.tree
                    .handle_input_event(terminal, crossterm_lock, event, can_compose)
                    .await
            }
        }
    }

    pub async fn cursor(&self) -> Option<M::Id> {
        match self.mode {
            Mode::Tree => self.tree.cursor().await,
        }
    }

    /// A [`Reaction::Composed`] message was sent, either successfully or
    /// unsuccessfully.
    ///
    /// If successful, include the message's id as an argument. If unsuccessful,
    /// instead pass a `None`.
    pub async fn sent(&mut self, id: Option<M::Id>) {
        match self.mode {
            Mode::Tree => self.tree.sent(id).await,
        }
    }
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
    M: Msg + ChatMsg + Send + Sync,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
    S::Error: fmt::Display,
{
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Size {
        match self {
            Self::Tree(tree) => tree.size(widthdb, max_width, max_height).await,
        }
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        match *self {
            Self::Tree(tree) => Box::new(tree).render(frame).await,
        }
    }
}
