//! Moving the cursor around.

use toss::frame::{Frame, Size};

use crate::chat::Cursor;
use crate::store::{Msg, MsgStore};

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
        old_cursor_id: &M::Id,
        cursor: &mut Cursor<M::Id>,
    ) {
        if let Some(block) = old_blocks.find(&cursor.id) {
            // The cursor is still visible in the old blocks, so we just need to
            // adjust the proportion such that the blocks stay still.
            cursor.proportion = util::line_to_proportion(size.height, block.line);
        } else {
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

    pub async fn move_up() {
        todo!()
    }

    pub async fn move_down() {
        todo!()
    }

    pub async fn move_up_sibling() {
        todo!()
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
