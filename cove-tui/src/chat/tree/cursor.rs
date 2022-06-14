//! Moving the cursor around.

use toss::frame::{Frame, Size};

use crate::chat::Cursor;
use crate::store::{Msg, MsgStore, Tree};

use super::blocks::Blocks;
use super::{util, TreeView};

impl<M: Msg> TreeView<M> {
    async fn correct_cursor_offset<S: MsgStore<M>>(
        &mut self,
        room: &str,
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
            let old_path = store.path(room, old_cursor_id).await;
            let new_path = store.path(room, &cursor.id).await;
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
        let blocks = self
            .layout_blocks(room, store, Some(cursor), frame, size)
            .await;
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

    /// Move to the previous sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_prev_sibling<S: MsgStore<M>>(
        &self,
        room: &str,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) {
        if let Some(siblings) = tree.siblings(id) {
            let prev_sibling = siblings
                .iter()
                .zip(siblings.iter().skip(1))
                .find(|(_, s)| *s == id)
                .map(|(s, _)| s);
            if let Some(prev_sibling) = prev_sibling {
                *id = prev_sibling.clone();
            }
        } else {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_tree_id) = store.prev_tree(room, tree.root()).await {
                *tree = store.tree(room, &prev_tree_id).await;
                *id = prev_tree_id;
            }
        }
    }

    /// Move to the next sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_next_sibling<S: MsgStore<M>>(
        &self,
        room: &str,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) {
        if let Some(siblings) = tree.siblings(id) {
            let next_sibling = siblings
                .iter()
                .zip(siblings.iter().skip(1))
                .find(|(s, _)| *s == id)
                .map(|(_, s)| s);
            if let Some(next_sibling) = next_sibling {
                *id = next_sibling.clone();
            }
        } else {
            // We're at the root of our tree, so we need to move to the root of
            // the next tree.
            if let Some(next_tree_id) = store.next_tree(room, tree.root()).await {
                *tree = store.tree(room, &next_tree_id).await;
                *id = next_tree_id;
            }
        }
    }

    fn find_innermost_child(tree: &Tree<M>, id: &mut M::Id) {
        while let Some(children) = tree.children(id) {
            if let Some(child) = children.last() {
                *id = child.clone()
            } else {
                break;
            }
        }
    }

    /// Move to the previous message, or don't move if this is not possible.
    async fn find_prev_msg<S: MsgStore<M>>(
        &self,
        room: &str,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) {
        if let Some(siblings) = tree.siblings(id) {
            let prev_sibling = siblings
                .iter()
                .zip(siblings.iter().skip(1))
                .find(|(_, s)| *s == id)
                .map(|(s, _)| s);
            if let Some(prev_sibling) = prev_sibling {
                *id = prev_sibling.clone();
            } else {
                // We need to move up one parent and *not* down again. If there
                // was no parent, we should be in the `else` case below instead.
                if let Some(parent) = tree.parent(id) {
                    *id = parent;
                    return;
                }
            }
        } else {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_tree_id) = store.prev_tree(room, tree.root()).await {
                *tree = store.tree(room, &prev_tree_id).await;
                *id = prev_tree_id;
            }
        }

        // Now, we just need to move to the deepest and last child.
        Self::find_innermost_child(tree, id);
    }

    /// Move to the next message, or don't move if this is not possible.
    async fn find_next_msg<S: MsgStore<M>>(
        &self,
        room: &str,
        store: &S,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) {
        if let Some(children) = tree.children(id) {
            if let Some(child) = children.first() {
                *id = child.clone();
                return;
            }
        }

        if let Some(siblings) = tree.siblings(id) {
            let prev_sibling = siblings
                .iter()
                .zip(siblings.iter().skip(1))
                .find(|(_, s)| *s == id)
                .map(|(s, _)| s);
            if let Some(next_sibling) = prev_sibling {
                *id = prev_sibling.clone();
            } else {
                // We need to move up one parent and *not* down again. If there
                // was no parent, we should be in the `else` case below instead.
                if let Some(parent) = tree.msg(id).and_then(|m| m.parent()) {
                    *id = parent;
                    return;
                }
            }
        } else {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_tree_id) = store.prev_tree(room, tree.root()).await {
                *tree = store.tree(room, &prev_tree_id).await;
                *id = prev_tree_id;
            }
        }

        // Now, we just need to move to the deepest and last child.
        Self::find_innermost_child(tree, id);
    }

    pub async fn move_up<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(room, store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let mut tree = store.tree(room, &cursor.id).await;
            self.find_prev_msg(room, store, &mut tree, &mut cursor.id)
                .await;
        } else if let Some(last_tree) = store.last_tree(room).await {
            let tree = store.tree(room, &last_tree).await;
            let mut id = last_tree;
            Self::find_innermost_child(&tree, &mut id);
            *cursor = Some(Cursor {
                id,
                proportion: 1.0,
            });
        }

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(
                room,
                store,
                frame,
                size,
                &old_blocks,
                &old_cursor_id,
                cursor,
            )
            .await;
        }
    }

    pub async fn move_down<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(room, store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let mut tree = store.tree(room, &cursor.id).await;
            self.find_next_msg(room, store, &mut tree, &mut cursor.id)
                .await;
        }

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(
                room,
                store,
                frame,
                size,
                &old_blocks,
                &old_cursor_id,
                cursor,
            )
            .await;
        }
    }

    pub async fn move_up_sibling<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(room, store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let mut tree = store.tree(room, &cursor.id).await;
            self.find_prev_sibling(room, store, &mut tree, &mut cursor.id)
                .await;
        } else if let Some(last_tree) = store.last_tree(room).await {
            *cursor = Some(Cursor {
                id: last_tree,
                proportion: 1.0,
            });
        }

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(
                room,
                store,
                frame,
                size,
                &old_blocks,
                &old_cursor_id,
                cursor,
            )
            .await;
        }
    }

    pub async fn move_down_sibling() {
        todo!()
    }

    pub async fn move_older() {
        todo!()
    }

    pub async fn move_newer() {
        todo!()
    }

    // TODO move_older_unseen
    // TODO move_newer_unseen

    pub async fn move_to_prev_msg<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
    ) {
        let tree = if let Some(cursor) = cursor {
            let path = store.path(room, &cursor.id).await;
            let tree = store.tree(room, path.first()).await;
            if let Some(prev_sibling) = tree.prev_sibling(&cursor.id) {
                cursor.id = tree.last_child(prev_sibling.clone());
                return;
            } else if let Some(parent) = tree.parent(&cursor.id) {
                cursor.id = parent;
                return;
            } else {
                store.prev_tree(room, path.first()).await
            }
        } else {
            store.last_tree(room).await
        };

        if let Some(tree) = tree {
            let tree = store.tree(room, &tree).await;
            let cursor_id = tree.last_child(tree.root().clone());
            if let Some(cursor) = cursor {
                cursor.id = cursor_id;
            } else {
                *cursor = Some(Cursor {
                    id: cursor_id,
                    proportion: 1.0,
                });
            }
        }
    }

    pub async fn center_cursor(&mut self, cursor: &mut Option<Cursor<M::Id>>) {
        if let Some(cursor) = cursor {
            cursor.proportion = 0.5;
        }
    }
}
