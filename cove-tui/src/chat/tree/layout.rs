use toss::frame::{Frame, Size};

use crate::chat::Cursor;
use crate::store::{Msg, MsgStore, Tree};

use super::blocks::{Block, Blocks};
use super::util::{self, MIN_CONTENT_WIDTH};
use super::TreeView;

fn msg_to_block<M: Msg>(frame: &mut Frame, size: Size, msg: &M, indent: usize) -> Block<M::Id> {
    let nick = msg.nick();
    let content = msg.content();

    let content_width = size.width as i32 - util::after_nick(frame, indent, &nick);
    if content_width < MIN_CONTENT_WIDTH as i32 {
        Block::placeholder(msg.id(), indent).time(msg.time())
    } else {
        let content_width = content_width as usize;
        let lines = toss::split_at_indices(&content, &frame.wrap(&content, content_width));
        let lines = lines.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
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
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        blocks: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while blocks.top_line > 0 {
            let tree = store.tree(room, &tree_id).await;
            blocks.prepend(layout_tree(frame, size, tree));
            if let Some(prev_tree_id) = store.prev_tree(room, &tree_id).await {
                tree_id = prev_tree_id;
            } else {
                break;
            }
        }
    }

    pub async fn expand_blocks_down<S: MsgStore<M>>(
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        blocks: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while blocks.bottom_line < size.height as i32 {
            let tree = store.tree(room, &tree_id).await;
            blocks.append(layout_tree(frame, size, tree));
            if let Some(next_tree_id) = store.next_tree(room, &tree_id).await {
                tree_id = next_tree_id;
            } else {
                break;
            }
        }
    }

    pub async fn layout<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) -> Blocks<M::Id> {
        let height: i32 = size.height.into();
        if let Some(cursor) = cursor {
            // TODO Ensure focus lies on cursor path, otherwise unfocus
            // TODO Unfold all messages on path to cursor

            // Layout cursor subtree (with correct offsets)
            let cursor_path = store.path(room, &cursor.id).await;
            let cursor_tree_id = cursor_path.first();
            let cursor_tree = store.tree(room, cursor_tree_id).await;
            let mut layout = layout_tree(frame, size, cursor_tree);
            layout.calculate_offsets_with_cursor(cursor, height);

            // Expand upwards and downwards
            // TODO Don't do this if there is a focus
            if let Some(prev_tree) = store.prev_tree(room, cursor_tree_id).await {
                Self::expand_blocks_up(room, store, frame, size, &mut layout, prev_tree).await;
            }
            if let Some(next_tree) = store.next_tree(room, cursor_tree_id).await {
                Self::expand_blocks_down(room, store, frame, size, &mut layout, next_tree).await;
            }

            layout
        } else {
            // TODO Ensure there is no focus

            // Start at the bottom of the screen
            let mut layout = Blocks::new_below(height - 1);

            // Expand upwards until the edge of the screen
            if let Some(last_tree) = store.last_tree(room).await {
                Self::expand_blocks_up(room, store, frame, size, &mut layout, last_tree).await;
            }

            layout
        }
    }
}
