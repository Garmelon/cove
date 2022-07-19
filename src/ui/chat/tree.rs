// mod action;
mod blocks;
// mod cursor;
mod layout;
mod render;
mod util;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore};
use crate::ui::widgets::Widget;

use self::blocks::{Block, BlockBody, Blocks, MarkerBlock};

///////////
// State //
///////////

/// Position of a cursor that is displayed as the last child of its parent
/// message, or last thread if it has no parent.
#[derive(Debug, Clone, Copy)]
struct LastChild<I> {
    coming_from: Option<I>,
    after: Option<I>,
}

#[derive(Debug, Clone, Copy)]
enum Cursor<I> {
    /// No cursor visible because it is at the bottom of the chat history.
    ///
    /// See also [`Anchor::Bottom`].
    Bottom,
    /// The cursor points to a message.
    Msg(I),
    /// The cursor has turned into an editor because we're composing a new
    /// message.
    Compose(LastChild<I>),
    /// A placeholder message is being displayed for a message that was just
    /// sent by the user.
    ///
    /// Will be replaced by a [`Cursor::Msg`] as soon as the server replies to
    /// the send command with the sent message.
    Placeholder(LastChild<I>),
}

impl<I: Eq> Cursor<I> {
    fn matches_block(&self, block: &Block<I>) -> bool {
        match self {
            Self::Bottom => matches!(&block.body, BlockBody::Marker(MarkerBlock::Bottom)),
            Self::Msg(id) => matches!(&block.body, BlockBody::Msg(msg) if msg.id == *id),
            Self::Compose(lc) | Self::Placeholder(lc) => match &lc.after {
                Some(bid) => {
                    matches!(&block.body, BlockBody::Marker(MarkerBlock::After(aid)) if aid == bid)
                }
                None => matches!(&block.body, BlockBody::Marker(MarkerBlock::Bottom)),
            },
        }
    }
}

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,
    last_blocks: Blocks<M::Id>,
    last_cursor: Cursor<M::Id>,
    cursor: Cursor<M::Id>,
    /// Set to true if the chat should be scrolled such that the cursor is fully
    /// visible (if possible). If set to false, then the cursor itself is moved
    /// to a different message such that it remains visible.
    make_cursor_visible: bool,
    editor: (), // TODO
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            last_blocks: Blocks::new(),
            last_cursor: Cursor::Bottom,
            cursor: Cursor::Bottom,
            make_cursor_visible: false,
            editor: (),
        }
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
        guard.relayout(frame).await;
        guard.draw_blocks(frame);
    }
}
