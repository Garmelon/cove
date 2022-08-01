mod blocks;
mod tree;

use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::KeyEvent;
use parking_lot::FairMutex;
use time::OffsetDateTime;
use toss::frame::{Frame, Size};
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::store::{Msg, MsgStore};

use self::tree::{TreeView, TreeViewState};

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

    pub fn widget(&self, nick: String) -> Chat<M, S> {
        match self.mode {
            Mode::Tree => Chat::Tree(self.tree.widget(nick)),
        }
    }

    pub async fn handle_navigation(&mut self, event: KeyEvent) -> bool {
        match self.mode {
            Mode::Tree => self.tree.handle_navigation(event).await,
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
                    .handle_messaging(terminal, crossterm_lock, event)
                    .await
            }
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
    M: Msg + ChatMsg,
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
