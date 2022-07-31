use toss::frame::Frame;

use crate::store::{Msg, MsgStore, Path, Tree};
use crate::ui::chat::blocks::Block;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::text::Text;

use super::tree_blocks::{BlockId, Root, TreeBlocks};
use super::{widgets, Cursor, InnerTreeViewState};

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    async fn cursor_path(&self, cursor: &Cursor<M::Id>) -> Path<M::Id> {
        match cursor {
            Cursor::Msg(id) => self.store.path(id).await,
            Cursor::Bottom | Cursor::Editor(None) | Cursor::Pseudo(None) => {
                Path::new(vec![M::last_possible_id()])
            }
            Cursor::Editor(Some(parent)) | Cursor::Pseudo(Some(parent)) => {
                let mut path = self.store.path(parent).await;
                path.push(M::last_possible_id());
                path
            }
        }
    }

    fn cursor_line(&self, blocks: &TreeBlocks<M::Id>) -> i32 {
        if let Cursor::Bottom = self.cursor {
            // The value doesn't matter as it will always be ignored.
            0
        } else {
            blocks
                .blocks()
                .find(&BlockId::from_cursor(&self.cursor))
                .expect("cursor is visible")
                .top_line
        }
    }

    fn contains_cursor(&self, blocks: &TreeBlocks<M::Id>) -> bool {
        blocks
            .blocks()
            .find(&BlockId::from_cursor(&self.cursor))
            .is_some()
    }

    fn layout_subtree(
        &self,
        frame: &mut Frame,
        tree: &Tree<M>,
        indent: usize,
        id: &M::Id,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        // Ghost cursor in front, for positioning according to last cursor line
        if self.last_cursor.refers_to(id) {
            let block = Block::new(frame, BlockId::LastCursor, Empty);
            blocks.blocks_mut().push_back(block);
        }

        // Main message body
        let highlighted = self.cursor.refers_to(id);
        let widget = if let Some(msg) = tree.msg(id) {
            widgets::msg(highlighted, indent, msg)
        } else {
            widgets::msg_placeholder(highlighted, indent)
        };
        let block = Block::new(frame, BlockId::Msg(id.clone()), widget);
        blocks.blocks_mut().push_back(block);

        // Children, recursively
        if let Some(children) = tree.children(id) {
            for child in children {
                self.layout_subtree(frame, tree, indent + 1, child, blocks);
            }
        }

        // Trailing ghost cursor, for positioning according to last cursor line
        if self.last_cursor.refers_to_last_child_of(id) {
            let block = Block::new(frame, BlockId::LastCursor, Empty);
            blocks.blocks_mut().push_back(block);
        }

        // Trailing editor or pseudomessage
        if self.cursor.refers_to_last_child_of(id) {
            // TODO Render proper editor or pseudocursor
            let block = Block::new(frame, BlockId::Cursor, Text::new("TODO"));
            blocks.blocks_mut().push_back(block);
        }
    }

    fn layout_tree(&self, frame: &mut Frame, tree: Tree<M>) -> TreeBlocks<M::Id> {
        let root = Root::Tree(tree.root().clone());
        let mut blocks = TreeBlocks::new(root.clone(), root);
        self.layout_subtree(frame, &tree, 0, tree.root(), &mut blocks);
        blocks
    }

    fn layout_bottom(&self, frame: &mut Frame) -> TreeBlocks<M::Id> {
        let mut blocks = TreeBlocks::new(Root::Bottom, Root::Bottom);

        // Ghost cursor, for positioning according to last cursor line
        if let Cursor::Editor(None) | Cursor::Pseudo(None) = self.last_cursor {
            let block = Block::new(frame, BlockId::LastCursor, Empty);
            blocks.blocks_mut().push_back(block);
        }

        // Editor or pseudomessage
        if let Cursor::Editor(None) | Cursor::Pseudo(None) = self.cursor {
            // TODO Render proper editor or pseudocursor
            let block = Block::new(frame, BlockId::Cursor, Text::new("TODO"));
            blocks.blocks_mut().push_back(block);
        }

        blocks
    }

    async fn expand_to_top(&self, frame: &mut Frame, blocks: &mut TreeBlocks<M::Id>) {
        let top_line = 0;

        while blocks.blocks().top_line > top_line {
            let top_root = blocks.top_root();
            let prev_tree_id = match top_root {
                Root::Bottom => self.store.last_tree_id().await,
                Root::Tree(tree_id) => self.store.prev_tree_id(tree_id).await,
            };
            let prev_tree_id = match prev_tree_id {
                Some(tree_id) => tree_id,
                None => break,
            };
            let prev_tree = self.store.tree(&prev_tree_id).await;
            blocks.prepend(self.layout_tree(frame, prev_tree));
        }
    }

    async fn expand_to_bottom(&self, frame: &mut Frame, blocks: &mut TreeBlocks<M::Id>) {
        let bottom_line = frame.size().height as i32 - 1;

        while blocks.blocks().bottom_line < bottom_line {
            let bottom_root = blocks.bottom_root();
            let next_tree_id = match bottom_root {
                Root::Bottom => break,
                Root::Tree(tree_id) => self.store.prev_tree_id(tree_id).await,
            };
            if let Some(next_tree_id) = next_tree_id {
                let next_tree = self.store.tree(&next_tree_id).await;
                blocks.append(self.layout_tree(frame, next_tree));
            } else {
                blocks.append(self.layout_bottom(frame));
            }
        }
    }

    async fn fill_screen_and_clamp_scrolling(
        &self,
        frame: &mut Frame,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        let top_line = 0;
        let bottom_line = frame.size().height as i32 - 1;

        self.expand_to_top(frame, blocks).await;

        if blocks.blocks().top_line > top_line {
            blocks.blocks_mut().set_top_line(0);
        }

        self.expand_to_bottom(frame, blocks).await;

        if blocks.blocks().bottom_line < bottom_line {
            blocks.blocks_mut().set_bottom_line(bottom_line);
        }

        self.expand_to_top(frame, blocks).await;
    }

    async fn layout_last_cursor_seed(
        &self,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        match &self.last_cursor {
            Cursor::Bottom => {
                let mut blocks = self.layout_bottom(frame);

                let bottom_line = frame.size().height as i32 - 1;
                blocks.blocks_mut().set_bottom_line(bottom_line);

                blocks
            }
            Cursor::Editor(None) | Cursor::Pseudo(None) => {
                let mut blocks = self.layout_bottom(frame);

                blocks
                    .blocks_mut()
                    .recalculate_offsets(&BlockId::LastCursor, self.last_cursor_line);

                blocks
            }
            Cursor::Msg(_) | Cursor::Editor(Some(_)) | Cursor::Pseudo(Some(_)) => {
                let root = last_cursor_path.first();
                let tree = self.store.tree(root).await;
                let mut blocks = self.layout_tree(frame, tree);

                blocks
                    .blocks_mut()
                    .recalculate_offsets(&BlockId::LastCursor, self.last_cursor_line);

                blocks
            }
        }
    }

    async fn layout_cursor_seed(
        &self,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
        cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        let bottom_line = frame.size().height as i32 - 1;

        match &self.cursor {
            Cursor::Bottom | Cursor::Editor(None) | Cursor::Pseudo(None) => {
                let mut blocks = self.layout_bottom(frame);

                blocks.blocks_mut().set_bottom_line(bottom_line);

                blocks
            }
            Cursor::Msg(_) | Cursor::Editor(Some(_)) | Cursor::Pseudo(Some(_)) => {
                let root = cursor_path.first();
                let tree = self.store.tree(root).await;
                let mut blocks = self.layout_tree(frame, tree);

                let cursor_above_last = cursor_path < last_cursor_path;
                let cursor_line = if cursor_above_last { 0 } else { bottom_line };
                blocks
                    .blocks_mut()
                    .recalculate_offsets(&BlockId::from_cursor(&self.cursor), cursor_line);

                blocks
            }
        }
    }

    async fn layout_initial_seed(
        &self,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
        cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        if let Cursor::Bottom = self.cursor {
            self.layout_cursor_seed(frame, last_cursor_path, cursor_path)
                .await
        } else {
            self.layout_last_cursor_seed(frame, last_cursor_path).await
        }
    }

    pub async fn relayout(&mut self, frame: &mut Frame) -> TreeBlocks<M::Id> {
        // The basic idea is this:
        //
        // First, layout a full screen of blocks around self.last_cursor, using
        // self.last_cursor_line for offset positioning.
        //
        // Then, check if self.cursor is somewhere in these blocks. If it is, we
        // now know the position of our own cursor. If it is not, it has jumped
        // too far away from self.last_cursor and we'll need to render a new
        // full screen of blocks around self.cursor before proceeding, using the
        // cursor paths to determine the position of self.cursor on the screen.
        //
        // Now that we have a more-or-less accurate screen position of
        // self.cursor, we can perform the actual cursor logic, i.e. make the
        // cursor visible or move it so it is visible.
        //
        // This entire process is complicated by the different kinds of cursors.

        let last_cursor_path = self.cursor_path(&self.last_cursor).await;
        let cursor_path = self.cursor_path(&self.cursor).await;

        let mut blocks = self
            .layout_initial_seed(frame, &last_cursor_path, &cursor_path)
            .await;
        self.fill_screen_and_clamp_scrolling(frame, &mut blocks)
            .await;

        if !self.contains_cursor(&blocks) {
            blocks = self
                .layout_cursor_seed(frame, &last_cursor_path, &cursor_path)
                .await;
            self.fill_screen_and_clamp_scrolling(frame, &mut blocks)
                .await;
        }

        if self.make_cursor_visible {
            // self.make_cursor_visible(&mut blocks).await; // TODO
            self.fill_screen_and_clamp_scrolling(frame, &mut blocks)
                .await;
        } else {
            // self.move_cursor_so_it_is_visible(&mut blocks); // TODO
            self.fill_screen_and_clamp_scrolling(frame, &mut blocks)
                .await;
        }

        self.last_cursor = self.cursor.clone();
        self.last_cursor_line = self.cursor_line(&blocks);
        self.make_cursor_visible = false;

        blocks
    }
}
