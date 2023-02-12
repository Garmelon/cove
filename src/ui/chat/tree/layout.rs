use toss::frame::Frame;

use crate::store::{Msg, MsgStore, Path, Tree};
use crate::ui::chat::blocks::Block;
use crate::ui::widgets::empty::Empty;
use crate::ui::ChatMsg;

use super::tree_blocks::{BlockId, Root, TreeBlocks};
use super::{widgets, Correction, Cursor, InnerTreeViewState};

const SCROLLOFF: i32 = 2;
const MIN_CONTENT_HEIGHT: i32 = 10;

fn scrolloff(height: i32) -> i32 {
    let scrolloff = (height - MIN_CONTENT_HEIGHT).max(0) / 2;
    scrolloff.min(SCROLLOFF)
}

struct Context {
    nick: String,
    focused: bool,
}

impl<M: Msg + ChatMsg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    async fn cursor_path(&self, cursor: &Cursor<M::Id>) -> Path<M::Id> {
        match cursor {
            Cursor::Msg(id) => self.store.path(id).await,
            Cursor::Bottom
            | Cursor::Editor { parent: None, .. }
            | Cursor::Pseudo { parent: None, .. } => Path::new(vec![M::last_possible_id()]),
            Cursor::Editor {
                parent: Some(parent),
                ..
            }
            | Cursor::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let mut path = self.store.path(parent).await;
                path.push(M::last_possible_id());
                path
            }
        }
    }

    fn make_path_visible(&mut self, path: &Path<M::Id>) {
        for segment in path.parent_segments() {
            self.folded.remove(segment);
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
                .expect("no cursor found")
                .top_line
        }
    }

    fn contains_cursor(&self, blocks: &TreeBlocks<M::Id>) -> bool {
        blocks
            .blocks()
            .find(&BlockId::from_cursor(&self.cursor))
            .is_some()
    }

    fn editor_block(
        &self,
        context: &Context,
        frame: &mut Frame,
        indent: usize,
    ) -> Block<BlockId<M::Id>> {
        let (widget, cursor_row) =
            widgets::editor::<M>(frame.widthdb(), indent, &context.nick, &self.editor);
        let cursor_row = cursor_row as i32;
        Block::new(frame, BlockId::Cursor, widget).focus(cursor_row..cursor_row + 1)
    }

    fn pseudo_block(
        &self,
        context: &Context,
        frame: &mut Frame,
        indent: usize,
    ) -> Block<BlockId<M::Id>> {
        let widget = widgets::pseudo::<M>(indent, &context.nick, &self.editor);
        Block::new(frame, BlockId::Cursor, widget)
    }

    fn layout_subtree(
        &self,
        context: &Context,
        frame: &mut Frame,
        tree: &Tree<M>,
        indent: usize,
        id: &M::Id,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        // Ghost cursor in front, for positioning according to last cursor line
        if self.last_cursor.refers_to(id) {
            let block = Block::new(frame, BlockId::LastCursor, Empty::new());
            blocks.blocks_mut().push_back(block);
        }

        // Last part of message body if message is folded
        let folded = self.folded.contains(id);
        let folded_info = if folded {
            Some(tree.subtree_size(id)).filter(|s| *s > 0)
        } else {
            None
        };

        // Main message body
        let highlighted = context.focused && self.cursor.refers_to(id);
        let widget = if let Some(msg) = tree.msg(id) {
            widgets::msg(highlighted, indent, msg, folded_info, self.config)
        } else {
            widgets::msg_placeholder(highlighted, indent, folded_info)
        };
        let block = Block::new(frame, BlockId::Msg(id.clone()), widget);
        blocks.blocks_mut().push_back(block);

        // Children, recursively
        if !folded {
            if let Some(children) = tree.children(id) {
                for child in children {
                    self.layout_subtree(context, frame, tree, indent + 1, child, blocks);
                }
            }
        }

        // Trailing ghost cursor, for positioning according to last cursor line
        if self.last_cursor.refers_to_last_child_of(id) {
            let block = Block::new(frame, BlockId::LastCursor, Empty::new());
            blocks.blocks_mut().push_back(block);
        }

        // Trailing editor or pseudomessage
        if self.cursor.refers_to_last_child_of(id) {
            match self.cursor {
                Cursor::Editor { .. } => {
                    blocks
                        .blocks_mut()
                        .push_back(self.editor_block(context, frame, indent + 1))
                }
                Cursor::Pseudo { .. } => {
                    blocks
                        .blocks_mut()
                        .push_back(self.pseudo_block(context, frame, indent + 1))
                }
                _ => {}
            }
        }
    }

    fn layout_tree(
        &self,
        context: &Context,
        frame: &mut Frame,
        tree: Tree<M>,
    ) -> TreeBlocks<M::Id> {
        let root = Root::Tree(tree.root().clone());
        let mut blocks = TreeBlocks::new(root.clone(), root);
        self.layout_subtree(context, frame, &tree, 0, tree.root(), &mut blocks);
        blocks
    }

    fn layout_bottom(&self, context: &Context, frame: &mut Frame) -> TreeBlocks<M::Id> {
        let mut blocks = TreeBlocks::new(Root::Bottom, Root::Bottom);

        // Ghost cursor, for positioning according to last cursor line
        if let Cursor::Editor { parent: None, .. } | Cursor::Pseudo { parent: None, .. } =
            self.last_cursor
        {
            let block = Block::new(frame, BlockId::LastCursor, Empty::new());
            blocks.blocks_mut().push_back(block);
        }

        match self.cursor {
            Cursor::Bottom => {
                let block = Block::new(frame, BlockId::Cursor, Empty::new());
                blocks.blocks_mut().push_back(block);
            }
            Cursor::Editor { parent: None, .. } => blocks
                .blocks_mut()
                .push_back(self.editor_block(context, frame, 0)),
            Cursor::Pseudo { parent: None, .. } => blocks
                .blocks_mut()
                .push_back(self.pseudo_block(context, frame, 0)),
            _ => {}
        }

        blocks
    }

    async fn expand_to_top(
        &self,
        context: &Context,
        frame: &mut Frame,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        let top_line = 0;

        while blocks.blocks().top_line > top_line {
            let top_root = blocks.top_root();
            let prev_root_id = match top_root {
                Root::Bottom => self.store.last_root_id().await,
                Root::Tree(root_id) => self.store.prev_root_id(root_id).await,
            };
            let prev_root_id = match prev_root_id {
                Some(id) => id,
                None => break,
            };
            let prev_tree = self.store.tree(&prev_root_id).await;
            blocks.prepend(self.layout_tree(context, frame, prev_tree));
        }
    }

    async fn expand_to_bottom(
        &self,
        context: &Context,
        frame: &mut Frame,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        let bottom_line = frame.size().height as i32 - 1;

        while blocks.blocks().bottom_line < bottom_line {
            let bottom_root = blocks.bottom_root();
            let next_root_id = match bottom_root {
                Root::Bottom => break,
                Root::Tree(root_id) => self.store.next_root_id(root_id).await,
            };
            if let Some(next_root_id) = next_root_id {
                let next_tree = self.store.tree(&next_root_id).await;
                blocks.append(self.layout_tree(context, frame, next_tree));
            } else {
                blocks.append(self.layout_bottom(context, frame));
            }
        }
    }

    async fn fill_screen_and_clamp_scrolling(
        &self,
        context: &Context,
        frame: &mut Frame,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        let top_line = 0;
        let bottom_line = frame.size().height as i32 - 1;

        self.expand_to_top(context, frame, blocks).await;

        if blocks.blocks().top_line > top_line {
            blocks.blocks_mut().set_top_line(0);
        }

        self.expand_to_bottom(context, frame, blocks).await;

        if blocks.blocks().bottom_line < bottom_line {
            blocks.blocks_mut().set_bottom_line(bottom_line);
        }

        self.expand_to_top(context, frame, blocks).await;
    }

    async fn layout_last_cursor_seed(
        &self,
        context: &Context,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        match &self.last_cursor {
            Cursor::Bottom => {
                let mut blocks = self.layout_bottom(context, frame);

                let bottom_line = frame.size().height as i32 - 1;
                blocks.blocks_mut().set_bottom_line(bottom_line);

                blocks
            }
            Cursor::Editor { parent: None, .. } | Cursor::Pseudo { parent: None, .. } => {
                let mut blocks = self.layout_bottom(context, frame);

                blocks
                    .blocks_mut()
                    .recalculate_offsets(&BlockId::LastCursor, self.last_cursor_line);

                blocks
            }
            Cursor::Msg(_)
            | Cursor::Editor {
                parent: Some(_), ..
            }
            | Cursor::Pseudo {
                parent: Some(_), ..
            } => {
                let root = last_cursor_path.first();
                let tree = self.store.tree(root).await;
                let mut blocks = self.layout_tree(context, frame, tree);

                blocks
                    .blocks_mut()
                    .recalculate_offsets(&BlockId::LastCursor, self.last_cursor_line);

                blocks
            }
        }
    }

    async fn layout_cursor_seed(
        &self,
        context: &Context,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
        cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        let bottom_line = frame.size().height as i32 - 1;

        match &self.cursor {
            Cursor::Bottom
            | Cursor::Editor { parent: None, .. }
            | Cursor::Pseudo { parent: None, .. } => {
                let mut blocks = self.layout_bottom(context, frame);

                blocks.blocks_mut().set_bottom_line(bottom_line);

                blocks
            }
            Cursor::Msg(_)
            | Cursor::Editor {
                parent: Some(_), ..
            }
            | Cursor::Pseudo {
                parent: Some(_), ..
            } => {
                let root = cursor_path.first();
                let tree = self.store.tree(root).await;
                let mut blocks = self.layout_tree(context, frame, tree);

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
        context: &Context,
        frame: &mut Frame,
        last_cursor_path: &Path<M::Id>,
        cursor_path: &Path<M::Id>,
    ) -> TreeBlocks<M::Id> {
        if let Cursor::Bottom = self.cursor {
            self.layout_cursor_seed(context, frame, last_cursor_path, cursor_path)
                .await
        } else {
            self.layout_last_cursor_seed(context, frame, last_cursor_path)
                .await
        }
    }

    fn scroll_so_cursor_is_visible(&self, frame: &Frame, blocks: &mut TreeBlocks<M::Id>) {
        if matches!(self.cursor, Cursor::Bottom) {
            return; // Cursor is locked to bottom
        }

        let block = blocks
            .blocks()
            .find(&BlockId::from_cursor(&self.cursor))
            .expect("no cursor found");

        let height = frame.size().height as i32;
        let scrolloff = scrolloff(height);

        let min_line = -block.focus.start + scrolloff;
        let max_line = height - block.focus.end - scrolloff;

        // If the message is higher than the available space, the top of the
        // message should always be visible. I'm not using top_line.clamp(...)
        // because the order of the min and max matters.
        let top_line = block.top_line;
        #[allow(clippy::manual_clamp)]
        let new_top_line = top_line.min(max_line).max(min_line);
        if new_top_line != top_line {
            blocks.blocks_mut().offset(new_top_line - top_line);
        }
    }

    fn scroll_so_cursor_is_centered(&self, frame: &Frame, blocks: &mut TreeBlocks<M::Id>) {
        if matches!(self.cursor, Cursor::Bottom) {
            return; // Cursor is locked to bottom
        }

        let block = blocks
            .blocks()
            .find(&BlockId::from_cursor(&self.cursor))
            .expect("no cursor found");

        let height = frame.size().height as i32;
        let scrolloff = scrolloff(height);

        let min_line = -block.focus.start + scrolloff;
        let max_line = height - block.focus.end - scrolloff;

        // If the message is higher than the available space, the top of the
        // message should always be visible. I'm not using top_line.clamp(...)
        // because the order of the min and max matters.
        let top_line = block.top_line;
        let new_top_line = (height - block.height) / 2;
        #[allow(clippy::manual_clamp)]
        let new_top_line = new_top_line.min(max_line).max(min_line);
        if new_top_line != top_line {
            blocks.blocks_mut().offset(new_top_line - top_line);
        }
    }

    /// Try to obtain a [`Cursor::Msg`] pointing to the block.
    fn msg_id(block: &Block<BlockId<M::Id>>) -> Option<M::Id> {
        match &block.id {
            BlockId::Msg(id) => Some(id.clone()),
            _ => None,
        }
    }

    fn visible(block: &Block<BlockId<M::Id>>, first_line: i32, last_line: i32) -> bool {
        (first_line + 1 - block.height..=last_line).contains(&block.top_line)
    }

    fn move_cursor_so_it_is_visible(
        &mut self,
        frame: &Frame,
        blocks: &TreeBlocks<M::Id>,
    ) -> Option<M::Id> {
        if !matches!(self.cursor, Cursor::Bottom | Cursor::Msg(_)) {
            // In all other cases, there is no need to make the cursor visible
            // since scrolling behaves differently enough.
            return None;
        }

        let height = frame.size().height as i32;
        let scrolloff = scrolloff(height);

        let first_line = scrolloff;
        let last_line = height - 1 - scrolloff;

        let new_cursor = if matches!(self.cursor, Cursor::Bottom) {
            blocks
                .blocks()
                .iter()
                .rev()
                .filter(|b| Self::visible(b, first_line, last_line))
                .find_map(Self::msg_id)
        } else {
            let block = blocks
                .blocks()
                .find(&BlockId::from_cursor(&self.cursor))
                .expect("no cursor found");

            if Self::visible(block, first_line, last_line) {
                return None;
            } else if block.top_line < first_line {
                blocks
                    .blocks()
                    .iter()
                    .filter(|b| Self::visible(b, first_line, last_line))
                    .find_map(Self::msg_id)
            } else {
                blocks
                    .blocks()
                    .iter()
                    .rev()
                    .filter(|b| Self::visible(b, first_line, last_line))
                    .find_map(Self::msg_id)
            }
        };

        if let Some(id) = new_cursor {
            self.cursor = Cursor::Msg(id.clone());
            Some(id)
        } else {
            None
        }
    }

    fn visible_msgs(frame: &Frame, blocks: &TreeBlocks<M::Id>) -> Vec<M::Id> {
        let height: i32 = frame.size().height.into();
        let first_line = 0;
        let last_line = first_line + height - 1;

        let mut result = vec![];
        for block in blocks.blocks().iter() {
            if Self::visible(block, first_line, last_line) {
                if let BlockId::Msg(id) = &block.id {
                    result.push(id.clone());
                }
            }
        }

        result
    }

    pub async fn relayout(
        &mut self,
        nick: String,
        focused: bool,
        frame: &mut Frame,
    ) -> TreeBlocks<M::Id> {
        // The basic idea is this:
        //
        // First, layout a full screen of blocks around self.last_cursor, using
        // self.last_cursor_line for offset positioning. At this point, any
        // outstanding scrolling is performed as well.
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

        let context = Context { nick, focused };

        let last_cursor_path = self.cursor_path(&self.last_cursor).await;
        let cursor_path = self.cursor_path(&self.cursor).await;
        self.make_path_visible(&cursor_path);

        let mut blocks = self
            .layout_initial_seed(&context, frame, &last_cursor_path, &cursor_path)
            .await;
        blocks.blocks_mut().offset(self.scroll);
        self.fill_screen_and_clamp_scrolling(&context, frame, &mut blocks)
            .await;

        if !self.contains_cursor(&blocks) {
            blocks = self
                .layout_cursor_seed(&context, frame, &last_cursor_path, &cursor_path)
                .await;
            self.fill_screen_and_clamp_scrolling(&context, frame, &mut blocks)
                .await;
        }

        match self.correction {
            Some(Correction::MakeCursorVisible) => {
                self.scroll_so_cursor_is_visible(frame, &mut blocks);
                self.fill_screen_and_clamp_scrolling(&context, frame, &mut blocks)
                    .await;
            }
            Some(Correction::MoveCursorToVisibleArea) => {
                let new_cursor_msg_id = self.move_cursor_so_it_is_visible(frame, &blocks);
                if let Some(cursor_msg_id) = new_cursor_msg_id {
                    // Moving the cursor invalidates our current blocks, so we sadly
                    // have to either perform an expensive operation or redraw the
                    // entire thing. I'm choosing the latter for now.

                    self.last_cursor = self.cursor.clone();
                    self.last_cursor_line = self.cursor_line(&blocks);
                    self.last_visible_msgs = Self::visible_msgs(frame, &blocks);
                    self.scroll = 0;
                    self.correction = None;

                    let last_cursor_path = self.store.path(&cursor_msg_id).await;
                    blocks = self
                        .layout_last_cursor_seed(&context, frame, &last_cursor_path)
                        .await;
                    self.fill_screen_and_clamp_scrolling(&context, frame, &mut blocks)
                        .await;
                }
            }
            Some(Correction::CenterCursor) => {
                self.scroll_so_cursor_is_centered(frame, &mut blocks);
                self.fill_screen_and_clamp_scrolling(&context, frame, &mut blocks)
                    .await;
            }
            None => {}
        }

        self.last_cursor = self.cursor.clone();
        self.last_cursor_line = self.cursor_line(&blocks);
        self.last_visible_msgs = Self::visible_msgs(frame, &blocks);
        self.scroll = 0;
        self.correction = None;

        blocks
    }
}
