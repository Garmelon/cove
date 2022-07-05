//! Moving the cursor around.

use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore, Tree};

use super::blocks::Blocks;
use super::{util, Cursor, TreeView};

impl<M: Msg> TreeView<M> {
    #[allow(clippy::too_many_arguments)]
    async fn correct_cursor_offset<S: MsgStore<M>>(
        &mut self,
        store: &S,
        frame: &mut Frame,
        size: Size,
        old_blocks: &Blocks<M::Id>,
        old_cursor_id: &Option<M::Id>,
        cursor: &mut Cursor<M::Id>,
    ) {
        if let Some(block) = old_blocks.find(&cursor.id) {
            // The cursor is still visible in the old blocks, so we just need to
            // adjust the proportion such that the blocks stay still.
            cursor.proportion = util::line_to_proportion(size.height, block.line);
        } else if let Some(old_cursor_id) = old_cursor_id {
            // The cursor is not visible any more. However, we can estimate
            // whether it is above or below the previous cursor position by
            // lexicographically comparing both positions' paths.
            let old_path = store.path(old_cursor_id).await;
            let new_path = store.path(&cursor.id).await;
            if new_path < old_path {
                // Because we moved upwards, the cursor should appear at the top
                // of the screen.
                cursor.proportion = 0.0;
            } else {
                // Because we moved downwards, the cursor should appear at the
                // bottom of the screen.
                cursor.proportion = 1.0;
            }
        } else {
            // We were scrolled all the way to the bottom, so the cursor must
            // have been offscreen somewhere above.
            cursor.proportion = 0.0;
        }

        // The cursor should be visible in its entirety on the screen now. If it
        // isn't, we need to scroll the screen such that the cursor becomes fully
        // visible again. To do this, we'll need to re-layout because the cursor
        // could've moved anywhere.
        let blocks = self.layout_blocks(store, Some(cursor), frame, size).await;
        let cursor_block = blocks.find(&cursor.id).expect("cursor must be in blocks");
        // First, ensure the cursor's last line is not below the bottom of the
        // screen. Then, ensure its top line is not above the top of the screen.
        // If the cursor has more lines than the screen, the user should still
        // see the top of the cursor so they can start reading its contents.
        let min_line = 0;
        let max_line = size.height as i32 - cursor_block.height;
        // Not using clamp because it is possible that max_line < min_line
        let cursor_line = cursor_block.line.min(max_line).max(min_line);
        cursor.proportion = util::line_to_proportion(size.height, cursor_line);

        // There is no need to ensure the screen is not scrolled too far up or
        // down. The messages in `blocks` are already scrolled correctly and
        // this function will not scroll the wrong way. If the cursor moves too
        // far up, the screen will only scroll down, not further up. The same
        // goes for the other direction.
    }

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
    async fn find_prev_sibling<S: MsgStore<M>>(
        &self,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
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
    async fn find_next_sibling<S: MsgStore<M>>(
        &self,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
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
    async fn find_prev_msg<S: MsgStore<M>>(
        &self,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
        // Move to previous sibling, then to its last child
        // If not possible, move to parent
        if self.find_prev_sibling(store, tree, id).await {
            while Self::find_last_child(tree, id) {}
            true
        } else {
            Self::find_parent(tree, id)
        }
    }

    /// Move to the next message, or don't move if this is not possible.
    async fn find_next_msg<S: MsgStore<M>>(
        &self,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
        if Self::find_first_child(tree, id) {
            return true;
        }

        if self.find_next_sibling(store, tree, id).await {
            return true;
        }

        // Temporary id to avoid modifying the original one if no parent-sibling
        // can be found.
        let mut tmp_id = id.clone();
        while Self::find_parent(tree, &mut tmp_id) {
            if self.find_next_sibling(store, tree, &mut tmp_id).await {
                *id = tmp_id;
                return true;
            }
        }

        false
    }

    pub async fn move_up<S: MsgStore<M>>(
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
            // We have a cursor to move around
            let path = store.path(&cursor.id).await;
            let mut tree = store.tree(path.first()).await;
            self.find_prev_msg(store, &mut tree, &mut cursor.id).await;
        } else if let Some(last_tree) = store.last_tree().await {
            // We need to select the last message of the last tree
            let tree = store.tree(&last_tree).await;
            let mut id = last_tree;
            while Self::find_last_child(&tree, &mut id) {}
            *cursor = Some(Cursor::new(id));
        }
        // If neither condition holds, we can't set a cursor because there's no
        // message to move to.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    pub async fn move_down<S: MsgStore<M>>(
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
            self.find_next_msg(store, &mut tree, &mut cursor.id).await;
        }
        // If that condition doesn't hold, we're already at the bottom in
        // cursor-less mode and can't move further down anyways.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

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

    pub async fn move_to_first<S: MsgStore<M>>(
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

        if let Some(tree_id) = store.first_tree().await {
            *cursor = Some(Cursor::new(tree_id));
        }

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    pub async fn move_to_last<S: MsgStore<M>>(
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

        if let Some(tree_id) = store.last_tree().await {
            let tree = store.tree(&tree_id).await;
            let mut id = tree_id;
            while Self::find_last_child(&tree, &mut id) {}
            *cursor = Some(Cursor::new(id));
        }

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
}
