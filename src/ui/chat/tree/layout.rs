//! Arranging messages as blocks.

use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore, Tree};

use super::blocks::{Block, Blocks};
use super::util::{self, MIN_CONTENT_WIDTH};
use super::{Cursor, TreeView};

fn msg_to_block<M: Msg>(frame: &mut Frame, size: Size, msg: &M, indent: usize) -> Block<M::Id> {
    let nick = msg.nick();
    let content = msg.content();

    let content_width = size.width as i32 - util::after_nick(frame, indent, &nick.text());
    if content_width < MIN_CONTENT_WIDTH as i32 {
        Block::placeholder(msg.id(), indent).time(msg.time())
    } else {
        let content_width = content_width as usize;
        let breaks = frame.wrap(&content.text(), content_width);
        let lines = content.split_at_indices(&breaks);
        Block::msg(msg.id(), indent, msg.time(), nick, lines)
    }
}

fn layout_subtree<M: Msg>(
    frame: &mut Frame,
    size: Size,
    tree: &Tree<M>,
    indent: usize,
    id: &M::Id,
    result: &mut Blocks<M::Id>,
) {
    let block = if let Some(msg) = tree.msg(id) {
        msg_to_block(frame, size, msg, indent)
    } else {
        Block::placeholder(id.clone(), indent)
    };
    result.push_back(block);

    if let Some(children) = tree.children(id) {
        for child in children {
            layout_subtree(frame, size, tree, indent + 1, child, result);
        }
    }
}

fn layout_tree<M: Msg>(frame: &mut Frame, size: Size, tree: Tree<M>) -> Blocks<M::Id> {
    let mut blocks = Blocks::new();
    layout_subtree(frame, size, &tree, 0, tree.root(), &mut blocks);
    blocks
}

impl<M: Msg> TreeView<M> {
    pub async fn expand_blocks_up<S: MsgStore<M>>(
        store: &S,
        frame: &mut Frame,
        size: Size,
        blocks: &mut Blocks<M::Id>,
        tree_id: &mut Option<M::Id>,
    ) {
        while blocks.top_line > 0 {
            *tree_id = if let Some(tree_id) = tree_id {
                store.prev_tree(tree_id).await
            } else {
                break;
            };

            if let Some(tree_id) = tree_id {
                let tree = store.tree(tree_id).await;
                blocks.prepend(layout_tree(frame, size, tree));
            } else {
                break;
            }
        }
    }

    pub async fn expand_blocks_down<S: MsgStore<M>>(
        store: &S,
        frame: &mut Frame,
        size: Size,
        blocks: &mut Blocks<M::Id>,
        tree_id: &mut Option<M::Id>,
    ) {
        while blocks.bottom_line < size.height as i32 {
            *tree_id = if let Some(tree_id) = tree_id {
                store.next_tree(tree_id).await
            } else {
                break;
            };

            if let Some(tree_id) = tree_id {
                let tree = store.tree(tree_id).await;
                blocks.append(layout_tree(frame, size, tree));
            } else {
                break;
            }
        }
    }

    // TODO Split up based on cursor presence
    pub async fn layout_blocks<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: Option<&Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) -> Blocks<M::Id> {
        if let Some(cursor) = cursor {
            // TODO Ensure focus lies on cursor path, otherwise unfocus
            // TODO Unfold all messages on path to cursor

            // Layout cursor subtree (with correct offsets based on cursor)
            let cursor_path = store.path(&cursor.id).await;
            let cursor_tree_id = cursor_path.first();
            let cursor_tree = store.tree(cursor_tree_id).await;
            let mut blocks = layout_tree(frame, size, cursor_tree);
            blocks.calculate_offsets_with_cursor(cursor, size.height);

            // Expand upwards and downwards, ensuring the blocks are not
            // scrolled too far in any direction.
            //
            // If the blocks fill the screen, scrolling stops when the topmost
            // message is at the top of the screen or the bottommost message is
            // at the bottom. If they don't fill the screen, the bottommost
            // message should always be at the bottom.
            //
            // Because our helper functions always expand the blocks until they
            // reach the top or bottom of the screen, we can determine that
            // we're at the top/bottom if expansion stopped anywhere in the
            // middle of the screen.
            //
            // TODO Don't expand if there is a focus
            let mut top_tree_id = Some(cursor_tree_id.clone());
            Self::expand_blocks_up(store, frame, size, &mut blocks, &mut top_tree_id).await;
            if blocks.top_line > 0 {
                blocks.offset(-blocks.top_line);
            }
            let mut bot_tree_id = Some(cursor_tree_id.clone());
            Self::expand_blocks_down(store, frame, size, &mut blocks, &mut bot_tree_id).await;
            if blocks.bottom_line < size.height as i32 - 1 {
                blocks.offset(size.height as i32 - 1 - blocks.bottom_line);
            }
            // If we only moved the blocks down, we need to expand upwards again
            // to make sure we fill the screen.
            Self::expand_blocks_up(store, frame, size, &mut blocks, &mut top_tree_id).await;

            blocks
        } else {
            // TODO Ensure there is no focus

            // Start at the bottom of the screen
            let mut blocks = Blocks::new_below(size.height as i32 - 1);

            // Expand upwards from last tree
            if let Some(last_tree_id) = store.last_tree().await {
                let last_tree = store.tree(&last_tree_id).await;
                blocks.prepend(layout_tree(frame, size, last_tree));

                let mut tree_id = Some(last_tree_id);
                Self::expand_blocks_up(store, frame, size, &mut blocks, &mut tree_id).await;
            }

            blocks
        }
    }
}
