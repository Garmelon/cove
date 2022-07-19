//! Arranging messages as blocks.

use toss::frame::{Frame, Size};

use crate::store::{Msg, MsgStore, Path, Tree};

use super::blocks::{Block, BlockBody, Blocks, MarkerBlock};
use super::{util, Cursor, InnerTreeViewState};

/*
impl<M: Msg> TreeView<M> {
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
*/

impl<I: Eq> Cursor<I> {
    fn matches_block(&self, block: &Block<I>) -> bool {
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
            Cursor::Msg(id) => Some(cursor_path.first()),
            Cursor::Compose(lc) | Cursor::Placeholder(lc) => match &lc.after {
                None => None,
                Some(id) => Some(cursor_path.first()),
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

        let content_width = size.width as i32 - util::after_nick(frame, indent, &nick.text());
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
    pub async fn layout_cursor_surroundings(&self, frame: &mut Frame) -> Blocks<M::Id> {
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

    pub async fn expand_blocks_up(&self, frame: &mut Frame, blocks: &mut Blocks<M::Id>) {
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

    pub async fn expand_blocks_down(&self, frame: &mut Frame, blocks: &mut Blocks<M::Id>) {
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
}
