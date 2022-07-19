// mod action;
mod blocks;
// mod cursor;
mod layout;
// mod render;
mod util;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore};
use crate::ui::widgets::Widget;

use self::blocks::Blocks;

///////////
// State //
///////////

/// Position of a cursor that is displayed as the last child of its parent
/// message, or last thread if it has no parent.
struct LastChild<I> {
    coming_from: Option<I>,
    after: Option<I>,
}

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

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,
    last_blocks: Blocks<M::Id>,
    last_cursor: Cursor<M::Id>,
    cursor: Cursor<M::Id>,
    editor: (), // TODO
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            last_blocks: Blocks::new(),
            last_cursor: Cursor::Bottom,
            cursor: Cursor::Bottom,
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
    M::Id: Send,
    S: MsgStore<M> + Send + Sync,
{
    fn size(&self, _frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        Size::ZERO
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        // Determine current cursor position
        //   If cursor in last blocks, use that
        //   If cursor below last cursor, use last line
        //   Otherwise, use first line
        // Layout starting from cursor tree
        // Make cursor visible
        //   If cursor was moved last, scroll so it is fully visible
        //   Otherwise, move cursor so it is barely visible
        // Clamp scrolling and fill screen again
        // Update last layout and last cursor position
        // Draw layout to screen
        todo!()
    }
}
