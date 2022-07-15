// mod action;
mod blocks;
// mod cursor;
// mod layout;
// mod render;
// mod util;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore};
use crate::ui::widgets::Widget;

///////////
// State //
///////////

/// The anchor specifies a specific line in a room's history.
enum Anchor<I> {
    /// The bottom of the room's history stays fixed.
    Bottom,
    /// The top of a message stays fixed.
    Msg(I),
    /// The line after a message's subtree stays fixed.
    After(I),
}

struct Compose<I> {
    /// The message that the cursor was on when composing began, or `None` if it
    /// was [`Cursor::Bottom`].
    ///
    /// Used to jump back to the original position when composing is aborted
    /// because the editor may be moved during composing.
    coming_from: Option<I>,
    /// The parent message of this reply, or `None` if it will be a new
    /// top-level message.
    parent: Option<I>,
    // TODO Editor state
    // TODO Whether currently editing or moving cursor
}

struct Placeholder<I> {
    /// See [`Composing::coming_from`].
    coming_from: Option<I>,
    /// See [`Composing::parent`].
    after: Option<I>,
}

enum Cursor<I> {
    /// No cursor visible because it is at the bottom of the chat history.
    ///
    /// See also [`Anchor::Bottom`].
    Bottom,
    /// The cursor points to a message.
    ///
    /// See also [`Anchor::Msg`].
    Msg(I),
    /// The cursor has turned into an editor because we're composing a new
    /// message.
    ///
    /// See also [`Anchor::After`].
    Compose(Compose<I>),
    /// A placeholder message is being displayed for a message that was just
    /// sent by the user.
    ///
    /// Will be replaced by a [`Cursor::Msg`] as soon as the server replies to
    /// the send command with the sent message. Otherwise, it will
    ///
    /// See also [`Anchor::After`].
    Placeholder(Placeholder<I>),
}

struct InnerTreeViewState<M: Msg, S: MsgStore<M>> {
    store: S,
    anchor: Anchor<M::Id>,
    anchor_line: i32,
    cursor: Cursor<M::Id>,
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn new(store: S) -> Self {
        Self {
            store,
            anchor: Anchor::Bottom,
            anchor_line: 0,
            cursor: Cursor::Bottom,
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
        todo!()
    }
}
