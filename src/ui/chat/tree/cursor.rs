//! Moving the cursor around.

use crate::store::{Msg, MsgStore, Tree};

use super::blocks::{Block, BlockBody, MarkerBlock};
use super::InnerTreeViewState;

/// Position of a cursor that is displayed as the last child of its parent
/// message, or last thread if it has no parent.
#[derive(Debug, Clone, Copy)]
pub struct LastChild<I> {
    pub coming_from: Option<I>,
    pub after: Option<I>,
}

#[derive(Debug, Clone, Copy)]
pub enum Cursor<I> {
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
    pub fn matches_block(&self, block: &Block<I>) -> bool {
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

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn find_parent(tree: &Tree<M>, id: &mut M::Id) -> bool {
        if let Some(parent) = tree.parent(id) {
            *id = parent;
            true
        } else {
            false
        }
    }

    fn find_first_child(tree: &Tree<M>, id: &mut M::Id) -> bool {
        if let Some(child) = tree.children(id).and_then(|c| c.first()) {
            *id = child.clone();
            true
        } else {
            false
        }
    }

    fn find_last_child(tree: &Tree<M>, id: &mut M::Id) -> bool {
        if let Some(child) = tree.children(id).and_then(|c| c.last()) {
            *id = child.clone();
            true
        } else {
            false
        }
    }

    /// Move to the previous sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_prev_sibling(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        if let Some(prev_sibling) = tree.prev_sibling(id) {
            *id = prev_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_tree_id) = store.prev_tree(tree.root()).await {
                *tree = store.tree(&prev_tree_id).await;
                *id = prev_tree_id;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Move to the next sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_next_sibling(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        if let Some(next_sibling) = tree.next_sibling(id) {
            *id = next_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the next tree.
            if let Some(next_tree_id) = store.next_tree(tree.root()).await {
                *tree = store.tree(&next_tree_id).await;
                *id = next_tree_id;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Move to the previous message, or don't move if this is not possible.
    async fn find_prev_msg(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        // Move to previous sibling, then to its last child
        // If not possible, move to parent
        if Self::find_prev_sibling(store, tree, id).await {
            while Self::find_last_child(tree, id) {}
            true
        } else {
            Self::find_parent(tree, id)
        }
    }

    /// Move to the next message, or don't move if this is not possible.
    async fn find_next_msg(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        if Self::find_first_child(tree, id) {
            return true;
        }

        if Self::find_next_sibling(store, tree, id).await {
            return true;
        }

        // Temporary id to avoid modifying the original one if no parent-sibling
        // can be found.
        let mut tmp_id = id.clone();
        while Self::find_parent(tree, &mut tmp_id) {
            if Self::find_next_sibling(store, tree, &mut tmp_id).await {
                *id = tmp_id;
                return true;
            }
        }

        false
    }

    pub async fn move_cursor_up(&mut self) {
        match &mut self.cursor {
            Cursor::Bottom => {
                if let Some(last_tree_id) = self.store.last_tree().await {
                    let tree = self.store.tree(&last_tree_id).await;
                    let mut id = last_tree_id;
                    while Self::find_last_child(&tree, &mut id) {}
                    self.cursor = Cursor::Msg(id);
                }
            }
            Cursor::Msg(ref mut msg) => {
                let path = self.store.path(msg).await;
                let mut tree = self.store.tree(path.first()).await;
                Self::find_prev_msg(&self.store, &mut tree, msg).await;
            }
            _ => {}
        }
        self.make_cursor_visible = true;
    }

    pub async fn move_cursor_down(&mut self) {
        if let Cursor::Msg(ref mut msg) = &mut self.cursor {
            let path = self.store.path(msg).await;
            let mut tree = self.store.tree(path.first()).await;
            if !Self::find_next_msg(&self.store, &mut tree, msg).await {
                self.cursor = Cursor::Bottom;
            }
        }
        self.make_cursor_visible = true;
    }

    pub async fn move_cursor_to_top(&mut self) {
        if let Some(tree_id) = self.store.first_tree().await {
            self.cursor = Cursor::Msg(tree_id);
            self.make_cursor_visible = true;
        }
    }

    pub async fn move_cursor_to_bottom(&mut self) {
        self.cursor = Cursor::Bottom;
        // Not really necessary; only here for consistency with other methods
        self.make_cursor_visible = true;
    }
}

/*
    pub async fn move_up_sibling<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let path = store.path(&cursor.id).await;
            let mut tree = store.tree(path.first()).await;
            self.find_prev_sibling(store, &mut tree, &mut cursor.id)
                .await;
        } else if let Some(last_tree) = store.last_tree().await {
            // I think moving to the root of the last tree makes the most sense
            // here. Alternatively, we could just not move the cursor, but that
            // wouldn't be very useful.
            *cursor = Some(Cursor::new(last_tree));
        }
        // If neither condition holds, we can't set a cursor because there's no
        // message to move to.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    pub async fn move_down_sibling<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let path = store.path(&cursor.id).await;
            let mut tree = store.tree(path.first()).await;
            self.find_next_sibling(store, &mut tree, &mut cursor.id)
                .await;
        }
        // If that condition doesn't hold, we're already at the bottom in
        // cursor-less mode and can't move further down anyways.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    // TODO move_older[_unseen]
    // TODO move_newer[_unseen]

    pub async fn center_cursor<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        if let Some(cursor) = cursor {
            cursor.proportion = 0.5;

            // Correcting the offset just to make sure that this function
            // behaves nicely if the cursor has too many lines.
            let old_blocks = self.layout_blocks(store, Some(cursor), frame, size).await;
            let old_cursor_id = Some(cursor.id.clone());
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }
*/
