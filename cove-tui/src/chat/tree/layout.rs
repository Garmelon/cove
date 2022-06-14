use toss::frame::{Frame, Size};

use crate::chat::Cursor;
use crate::store::{Msg, MsgStore, Tree};

use super::blocks::{Block, Blocks, MsgBlock};
use super::constants::{INDENT_WIDTH, TIME_WIDTH};
use super::TreeView;

impl<M: Msg> TreeView<M> {
    fn msg_to_block(
        &mut self,
        msg: &M,
        indent: usize,
        frame: &mut Frame,
        size: Size,
    ) -> Block<M::Id> {
        let nick = msg.nick();
        let content = msg.content();

        let used_width = TIME_WIDTH + INDENT_WIDTH * indent + 1 + frame.width(&nick) + 2;
        let rest_width = size.width as usize - used_width;

        let lines = toss::split_at_indices(&content, &frame.wrap(&content, rest_width));
        let lines = lines.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
        MsgBlock {
            time: msg.time(),
            nick,
            lines,
        }
        .into_block(msg.id(), indent)
    }

    fn layout_subtree(
        &mut self,
        tree: &Tree<M>,
        frame: &mut Frame,
        size: Size,
        indent: usize,
        id: &M::Id,
        layout: &mut Blocks<M::Id>,
    ) {
        let block = if let Some(msg) = tree.msg(id) {
            self.msg_to_block(msg, indent, frame, size)
        } else {
            Block::placeholder(id.clone(), indent)
        };
        layout.push_back(block);

        if let Some(children) = tree.children(id) {
            for child in children {
                self.layout_subtree(tree, frame, size, indent + 1, child, layout);
            }
        }
    }

    fn layout_tree(&mut self, tree: Tree<M>, frame: &mut Frame, size: Size) -> Blocks<M::Id> {
        let mut layout = Blocks::new();
        self.layout_subtree(&tree, frame, size, 0, tree.root(), &mut layout);
        layout
    }

    async fn expand_layout_upwards<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        layout: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while layout.top_line > 0 {
            let tree = store.tree(room, &tree_id).await;
            layout.prepend(self.layout_tree(tree, frame, size));
            if let Some(prev_tree_id) = store.prev_tree(room, &tree_id).await {
                tree_id = prev_tree_id;
            } else {
                break;
            }
        }
    }

    async fn expand_layout_downwards<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: &S,
        frame: &mut Frame,
        size: Size,
        layout: &mut Blocks<M::Id>,
        mut tree_id: M::Id,
    ) {
        while layout.bottom_line < size.height as i32 {
            let tree = store.tree(room, &tree_id).await;
            layout.append(self.layout_tree(tree, frame, size));
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

            // Produce layout of cursor subtree (with correct offsets)
            let cursor_path = store.path(room, &cursor.id).await;
            let cursor_tree_id = cursor_path.first();
            let cursor_tree = store.tree(room, cursor_tree_id).await;
            let mut layout = self.layout_tree(cursor_tree, frame, size);
            layout.calculate_offsets_with_cursor(cursor, height);

            // Expand layout upwards and downwards
            // TODO Don't do this if there is a focus
            if let Some(prev_tree) = store.prev_tree(room, cursor_tree_id).await {
                self.expand_layout_upwards(room, store, frame, size, &mut layout, prev_tree)
                    .await;
            }
            if let Some(next_tree) = store.next_tree(room, cursor_tree_id).await {
                self.expand_layout_downwards(room, store, frame, size, &mut layout, next_tree)
                    .await;
            }

            layout
        } else {
            // TODO Ensure there is no focus

            // Start layout at the bottom of the screen
            let mut layout = Blocks::new_below(height - 1);

            // Expand layout upwards until the edge of the screen
            if let Some(last_tree) = store.last_tree(room).await {
                self.expand_layout_upwards(room, store, frame, size, &mut layout, last_tree)
                    .await;
            }

            layout
        }
    }
}
