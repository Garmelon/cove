//! Arranging messages as blocks.

use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore, Path, Tree};

use super::blocks::{Block, BlockBody, Blocks, MarkerBlock, MsgBlock};
use super::{util, Cursor, InnerTreeViewState};

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    async fn cursor_path(&self, cursor: &Cursor<M::Id>) -> Path<M::Id> {
        match cursor {
            Cursor::Bottom => match self.store.last_tree().await {
                Some(id) => Path::new(vec![id]),
                None => Path::new(vec![M::last_possible_id()]),
            },
            Cursor::Msg(id) => self.store.path(id).await,
            Cursor::Compose(lc) | Cursor::Placeholder(lc) => match &lc.after {
                None => Path::new(vec![M::last_possible_id()]),
                Some(id) => {
                    let mut path = self.store.path(id).await;
                    path.push(M::last_possible_id());
                    path
                }
            },
        }
    }

    fn cursor_tree_id<'a>(
        cursor: &Cursor<M::Id>,
        cursor_path: &'a Path<M::Id>,
    ) -> Option<&'a M::Id> {
        match cursor {
            Cursor::Bottom => None,
            Cursor::Msg(_) => Some(cursor_path.first()),
            Cursor::Compose(lc) | Cursor::Placeholder(lc) => match &lc.after {
                None => None,
                Some(_) => Some(cursor_path.first()),
            },
        }
    }

    fn cursor_line(
        last_blocks: &Blocks<M::Id>,
        cursor: &Cursor<M::Id>,
        cursor_path: &Path<M::Id>,
        last_cursor_path: &Path<M::Id>,
        size: Size,
    ) -> i32 {
        if let Some(block) = last_blocks.find(|b| cursor.matches_block(b)) {
            block.line
        } else if last_cursor_path < cursor_path {
            // Not using size.height - 1 because markers like
            // MarkerBlock::Bottom in the line below the last visible line are
            // still relevant to us.
            size.height.into()
        } else {
            0
        }
    }

    fn msg_to_block(frame: &mut Frame, indent: usize, msg: &M) -> Block<M::Id> {
        let size = frame.size();

        let nick = msg.nick();
        let content = msg.content();

        let content_width = size.width as i32 - util::after_nick(frame, indent, &nick);
        if content_width < util::MIN_CONTENT_WIDTH as i32 {
            Block::placeholder(Some(msg.time()), indent, msg.id())
        } else {
            let content_width = content_width as usize;
            let breaks = frame.wrap(&content.text(), content_width);
            let lines = content.split_at_indices(&breaks);
            Block::msg(msg.time(), indent, msg.id(), nick, lines)
        }
    }

    fn layout_subtree(
        frame: &mut Frame,
        tree: &Tree<M>,
        indent: usize,
        id: &M::Id,
        result: &mut Blocks<M::Id>,
    ) {
        let block = if let Some(msg) = tree.msg(id) {
            Self::msg_to_block(frame, indent, msg)
        } else {
            Block::placeholder(None, indent, id.clone())
        };
        result.push_back(block);

        if let Some(children) = tree.children(id) {
            for child in children {
                Self::layout_subtree(frame, tree, indent + 1, child, result);
            }
        }

        result.push_back(Block::after(indent, id.clone()))
    }

    fn layout_tree(frame: &mut Frame, tree: Tree<M>) -> Blocks<M::Id> {
        let mut blocks = Blocks::new();
        Self::layout_subtree(frame, &tree, 0, tree.root(), &mut blocks);
        blocks
    }

    /// Create a [`Blocks`] of the current cursor's immediate surroundings.
    async fn layout_cursor_surroundings(&self, frame: &mut Frame) -> Blocks<M::Id> {
        let size = frame.size();

        let cursor_path = self.cursor_path(&self.cursor).await;
        let last_cursor_path = self.cursor_path(&self.last_cursor).await;
        let tree_id = Self::cursor_tree_id(&self.cursor, &cursor_path);
        let cursor_line = Self::cursor_line(
            &self.last_blocks,
            &self.cursor,
            &cursor_path,
            &last_cursor_path,
            size,
        );

        if let Some(tree_id) = tree_id {
            let tree = self.store.tree(tree_id).await;
            let mut blocks = Self::layout_tree(frame, tree);
            blocks.recalculate_offsets(|b| {
                if self.cursor.matches_block(b) {
                    Some(cursor_line)
                } else {
                    None
                }
            });
            blocks
        } else {
            let mut blocks = Blocks::new_below(cursor_line);
            blocks.push_front(Block::bottom());
            blocks
        }
    }

    fn scroll_so_cursor_is_visible(blocks: &mut Blocks<M::Id>, cursor: &Cursor<M::Id>, size: Size) {
        if let Some(block) = blocks.find(|b| cursor.matches_block(b)) {
            let min_line = 0;
            let max_line = size.height as i32 - block.height();
            if block.line < min_line {
                blocks.offset(min_line - block.line);
            } else if block.line > max_line {
                blocks.offset(max_line - block.line);
            }
        } else {
            // This should never happen since we always start rendering the
            // blocks from the cursor.
            panic!("no cursor found");
        }
    }

    /// Try to obtain a normal cursor (i.e. no composing or placeholder cursor)
    /// pointing to the block.
    fn as_direct_cursor(block: &Block<M::Id>) -> Option<Cursor<M::Id>> {
        match &block.body {
            BlockBody::Marker(MarkerBlock::Bottom) => Some(Cursor::Bottom),
            BlockBody::Msg(MsgBlock { id, .. }) => Some(Cursor::Msg(id.clone())),
            _ => None,
        }
    }

    fn move_cursor_so_it_is_visible(
        blocks: &mut Blocks<M::Id>,
        cursor: &mut Cursor<M::Id>,
        size: Size,
    ) {
        if matches!(cursor, Cursor::Compose(_) | Cursor::Placeholder(_)) {
            // In this case, we can't easily move the cursor since moving it
            // would change how the entire layout is rendered in
            // difficult-to-predict ways.
            //
            // Also, the user has initiated a reply to get into this state. This
            // confirms that they want their cursor in precisely its current
            // place. Moving it might lead to mis-replies and frustration.
            return;
        }

        if let Some(block) = blocks.find(|b| cursor.matches_block(b)) {
            let min_line = 1 - block.height();
            let max_line = size.height as i32 - 1;

            let new_cursor = if block.line < min_line {
                // Move cursor to first possible visible block
                blocks
                    .iter()
                    .filter(|b| b.line >= min_line)
                    .find_map(Self::as_direct_cursor)
            } else if block.line > max_line {
                // Move cursor to last possible visible block
                blocks
                    .iter()
                    .rev()
                    .filter(|b| b.line <= max_line)
                    .find_map(Self::as_direct_cursor)
            } else {
                None
            };

            if let Some(new_cursor) = new_cursor {
                *cursor = new_cursor;
            }
        } else {
            // This should never happen since we always start rendering the
            // blocks from the cursor.
            panic!("no cursor found");
        }
    }

    async fn expand_blocks_up(&self, frame: &mut Frame, blocks: &mut Blocks<M::Id>) {
        while blocks.top_line > 0 {
            let tree_id = if let Some((root_top, _)) = &blocks.roots {
                self.store.prev_tree(root_top).await
            } else {
                self.store.last_tree().await
            };

            if let Some(tree_id) = tree_id {
                let tree = self.store.tree(&tree_id).await;
                blocks.prepend(Self::layout_tree(frame, tree));
            } else {
                break;
            }
        }
    }

    async fn expand_blocks_down(&self, frame: &mut Frame, blocks: &mut Blocks<M::Id>) {
        while blocks.bottom_line < frame.size().height as i32 {
            let tree_id = if let Some((_, root_bot)) = &blocks.roots {
                self.store.next_tree(root_bot).await
            } else {
                // We assume that a Blocks without roots is at the bottom of the
                // room's history. Therefore, there are no more messages below.
                break;
            };

            if let Some(tree_id) = tree_id {
                let tree = self.store.tree(&tree_id).await;
                blocks.append(Self::layout_tree(frame, tree));
            } else {
                break;
            }
        }
    }

    async fn clamp_scrolling(&self, frame: &mut Frame, blocks: &mut Blocks<M::Id>) {
        let size = frame.size();
        let top_line = 0;
        let bottom_line = size.height as i32 - 1;

        self.expand_blocks_up(frame, blocks).await;

        if blocks.top_line > top_line {
            blocks.offset(top_line - blocks.top_line);
        }

        self.expand_blocks_down(frame, blocks).await;

        if blocks.bottom_line < bottom_line {
            blocks.offset(bottom_line - blocks.bottom_line);
        }

        self.expand_blocks_up(frame, blocks).await;
    }

    pub async fn relayout(&mut self, frame: &mut Frame) {
        let size = frame.size();

        let mut blocks = self.layout_cursor_surroundings(frame).await;

        if self.make_cursor_visible {
            Self::scroll_so_cursor_is_visible(&mut blocks, &self.cursor, size);
        }

        self.clamp_scrolling(frame, &mut blocks).await;

        if !self.make_cursor_visible {
            Self::move_cursor_so_it_is_visible(&mut blocks, &mut self.cursor, size);
        }

        self.last_blocks = blocks;
        self.last_cursor = self.cursor.clone();
        self.make_cursor_visible = false;
    }
}
