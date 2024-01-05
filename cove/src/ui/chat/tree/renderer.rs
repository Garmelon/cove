//! A [`Renderer`] for message trees.

use std::collections::HashSet;
use std::convert::Infallible;

use async_trait::async_trait;
use toss::widgets::{EditorState, Empty, Predrawn, Resize};
use toss::{Size, Widget, WidthDb};

use crate::store::{Msg, MsgStore, Tree};
use crate::ui::chat::blocks::{Block, Blocks, Range};
use crate::ui::chat::cursor::Cursor;
use crate::ui::chat::renderer::{self, overlaps, Renderer};
use crate::ui::ChatMsg;
use crate::util::InfallibleExt;

use super::widgets;

/// When rendering messages as full trees, special ids and zero-height messages
/// are used for robust scrolling behaviour.
#[derive(PartialEq, Eq)]
pub enum TreeBlockId<Id> {
    /// There is a zero-height block at the very bottom of the chat that has
    /// this id. It is used for positioning [`Cursor::Bottom`].
    Bottom,
    /// Normal messages have this id. It is used for positioning
    /// [`Cursor::Msg`].
    Msg(Id),
    /// After all children of a message, a zero-height block with this id is
    /// rendered. It is used for positioning [`Cursor::Editor`] and
    /// [`Cursor::Pseudo`].
    After(Id),
}

impl<Id: Clone> TreeBlockId<Id> {
    pub fn from_cursor(cursor: &Cursor<Id>) -> Self {
        match cursor {
            Cursor::Bottom
            | Cursor::Editor { parent: None, .. }
            | Cursor::Pseudo { parent: None, .. } => Self::Bottom,

            Cursor::Msg(id) => Self::Msg(id.clone()),

            Cursor::Editor {
                parent: Some(id), ..
            }
            | Cursor::Pseudo {
                parent: Some(id), ..
            } => Self::After(id.clone()),
        }
    }

    pub fn any_id(&self) -> Option<&Id> {
        match self {
            Self::Bottom => None,
            Self::Msg(id) | Self::After(id) => Some(id),
        }
    }

    pub fn msg_id(&self) -> Option<&Id> {
        match self {
            Self::Bottom | Self::After(_) => None,
            Self::Msg(id) => Some(id),
        }
    }
}

type TreeBlock<Id> = Block<TreeBlockId<Id>>;
type TreeBlocks<Id> = Blocks<TreeBlockId<Id>>;

pub struct TreeContext<Id> {
    pub size: Size,
    pub nick: String,
    pub focused: bool,
    pub caesar: i8,
    pub last_cursor: Cursor<Id>,
    pub last_cursor_top: i32,
}

pub struct TreeRenderer<'a, M: Msg, S: MsgStore<M>> {
    context: TreeContext<M::Id>,

    store: &'a S,
    folded: &'a mut HashSet<M::Id>,
    cursor: &'a mut Cursor<M::Id>,
    editor: &'a mut EditorState,
    widthdb: &'a mut WidthDb,

    /// Root id of the topmost tree in the blocks. When set to `None`, only the
    /// bottom of the chat history has been rendered.
    top_root_id: Option<M::Id>,
    /// Root id of the bottommost tree in the blocks. When set to `None`, only
    /// the bottom of the chat history has been rendered.
    bottom_root_id: Option<M::Id>,

    blocks: TreeBlocks<M::Id>,
}

impl<'a, M, S> TreeRenderer<'a, M, S>
where
    M: Msg + ChatMsg + Send + Sync,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
    S::Error: Send,
{
    /// You must call [`Self::prepare_blocks_for_drawing`] immediately after
    /// calling this function.
    pub fn new(
        context: TreeContext<M::Id>,
        store: &'a S,
        folded: &'a mut HashSet<M::Id>,
        cursor: &'a mut Cursor<M::Id>,
        editor: &'a mut EditorState,
        widthdb: &'a mut WidthDb,
    ) -> Self {
        Self {
            context,
            store,
            folded,
            cursor,
            editor,
            widthdb,
            top_root_id: None,
            bottom_root_id: None,
            blocks: Blocks::new(0),
        }
    }

    fn predraw<W>(widget: W, size: Size, widthdb: &mut WidthDb) -> Predrawn
    where
        W: Widget<Infallible>,
    {
        Predrawn::new(Resize::new(widget).with_max_width(size.width), widthdb).infallible()
    }

    fn zero_height_block(&mut self, parent: Option<&M::Id>) -> TreeBlock<M::Id> {
        let id = match parent {
            Some(parent) => TreeBlockId::After(parent.clone()),
            None => TreeBlockId::Bottom,
        };

        let widget = Self::predraw(Empty::new(), self.context.size, self.widthdb);
        Block::new(id, widget, false)
    }

    fn editor_block(&mut self, indent: usize, parent: Option<&M::Id>) -> TreeBlock<M::Id> {
        let id = match parent {
            Some(parent) => TreeBlockId::After(parent.clone()),
            None => TreeBlockId::Bottom,
        };

        let widget = widgets::editor::<M>(
            indent,
            &self.context.nick,
            self.context.focused,
            self.editor,
        );
        let widget = Self::predraw(widget, self.context.size, self.widthdb);
        let mut block = Block::new(id, widget, false);

        // Since the editor was rendered when the `Predrawn` was created, the
        // last cursor pos is accurate now.
        let cursor_line = self.editor.last_cursor_pos().y;
        block.set_focus(Range::new(cursor_line, cursor_line + 1));

        block
    }

    fn pseudo_block(&mut self, indent: usize, parent: Option<&M::Id>) -> TreeBlock<M::Id> {
        let id = match parent {
            Some(parent) => TreeBlockId::After(parent.clone()),
            None => TreeBlockId::Bottom,
        };

        let widget = widgets::pseudo::<M>(indent, &self.context.nick, self.editor);
        let widget = Self::predraw(widget, self.context.size, self.widthdb);
        Block::new(id, widget, false)
    }

    fn message_block(
        &mut self,
        indent: usize,
        msg: &M,
        folded_info: Option<usize>,
    ) -> TreeBlock<M::Id> {
        let msg_id = msg.id();

        let highlighted = match self.cursor {
            Cursor::Msg(id) => *id == msg_id,
            _ => false,
        };
        let highlighted = highlighted && self.context.focused;

        let widget = widgets::msg(highlighted, indent, msg, self.context.caesar, folded_info);
        let widget = Self::predraw(widget, self.context.size, self.widthdb);
        Block::new(TreeBlockId::Msg(msg_id), widget, true)
    }

    fn message_placeholder_block(
        &mut self,
        indent: usize,
        msg_id: &M::Id,
        folded_info: Option<usize>,
    ) -> TreeBlock<M::Id> {
        let highlighted = match self.cursor {
            Cursor::Msg(id) => id == msg_id,
            _ => false,
        };
        let highlighted = highlighted && self.context.focused;

        let widget = widgets::msg_placeholder(highlighted, indent, folded_info);
        let widget = Self::predraw(widget, self.context.size, self.widthdb);
        Block::new(TreeBlockId::Msg(msg_id.clone()), widget, true)
    }

    fn layout_bottom(&mut self) -> TreeBlocks<M::Id> {
        let mut blocks = Blocks::new(0);

        match self.cursor {
            Cursor::Editor { parent: None, .. } => blocks.push_bottom(self.editor_block(0, None)),
            Cursor::Pseudo { parent: None, .. } => blocks.push_bottom(self.pseudo_block(0, None)),
            _ => blocks.push_bottom(self.zero_height_block(None)),
        }

        blocks
    }

    fn layout_subtree(
        &mut self,
        tree: &Tree<M>,
        indent: usize,
        msg_id: &M::Id,
        blocks: &mut TreeBlocks<M::Id>,
    ) {
        let folded = self.folded.contains(msg_id);
        let folded_info = if folded {
            Some(tree.subtree_size(msg_id)).filter(|s| *s > 0)
        } else {
            None
        };

        // Message itself
        let block = if let Some(msg) = tree.msg(msg_id) {
            self.message_block(indent, msg, folded_info)
        } else {
            self.message_placeholder_block(indent, msg_id, folded_info)
        };
        blocks.push_bottom(block);

        // Children, recursively
        if !folded {
            if let Some(children) = tree.children(msg_id) {
                for child in children {
                    self.layout_subtree(tree, indent + 1, child, blocks);
                }
            }
        }

        // After message (zero-height block, editor, or placeholder)
        let block = match self.cursor {
            Cursor::Editor {
                parent: Some(id), ..
            } if id == msg_id => self.editor_block(indent + 1, Some(msg_id)),

            Cursor::Pseudo {
                parent: Some(id), ..
            } if id == msg_id => self.pseudo_block(indent + 1, Some(msg_id)),

            _ => self.zero_height_block(Some(msg_id)),
        };
        blocks.push_bottom(block);
    }

    fn layout_tree(&mut self, tree: Tree<M>) -> TreeBlocks<M::Id> {
        let mut blocks = Blocks::new(0);
        self.layout_subtree(&tree, 0, tree.root(), &mut blocks);
        blocks
    }

    async fn root_id(&self, id: &TreeBlockId<M::Id>) -> Result<Option<M::Id>, S::Error> {
        let Some(id) = id.any_id() else {
            return Ok(None);
        };
        let path = self.store.path(id).await?;
        Ok(Some(path.into_first()))
    }

    /// Render the tree containing the cursor to the blocks and set the top and
    /// bottom root id accordingly. This function will always render a block
    /// that has the cusor id.
    async fn prepare_initial_tree(
        &mut self,
        cursor_id: &TreeBlockId<M::Id>,
        root_id: &Option<M::Id>,
    ) -> Result<(), S::Error> {
        self.top_root_id = root_id.clone();
        self.bottom_root_id = root_id.clone();

        let blocks = if let Some(root_id) = root_id {
            let tree = self.store.tree(root_id).await?;

            // To ensure the cursor block will be rendered, all its parents must
            // be unfolded.
            if let TreeBlockId::Msg(id) | TreeBlockId::After(id) = cursor_id {
                let mut id = id.clone();
                while let Some(parent_id) = tree.parent(&id) {
                    self.folded.remove(&parent_id);
                    id = parent_id;
                }
            }

            self.layout_tree(tree)
        } else {
            self.layout_bottom()
        };
        self.blocks.append_bottom(blocks);

        Ok(())
    }

    fn make_cursor_visible(&mut self) {
        let cursor_id = TreeBlockId::from_cursor(self.cursor);
        if *self.cursor == self.context.last_cursor {
            // Cursor did not move, so we just need to ensure it overlaps the
            // scroll area
            renderer::scroll_so_block_focus_overlaps_scroll_area(self, &cursor_id);
        } else {
            // Cursor moved, so it should fully overlap the scroll area
            renderer::scroll_so_block_focus_fully_overlaps_scroll_area(self, &cursor_id);
        }
    }

    fn root_id_is_above_root_id(first: Option<M::Id>, second: Option<M::Id>) -> bool {
        match (first, second) {
            (Some(_), None) => true,
            (Some(a), Some(b)) => a < b,
            _ => false,
        }
    }

    pub async fn prepare_blocks_for_drawing(&mut self) -> Result<(), S::Error> {
        let cursor_id = TreeBlockId::from_cursor(self.cursor);
        let cursor_root_id = self.root_id(&cursor_id).await?;

        // Render cursor and blocks around it so that the screen will always be
        // filled as long as the cursor is visible, regardless of how the screen
        // is scrolled.
        self.prepare_initial_tree(&cursor_id, &cursor_root_id)
            .await?;
        renderer::expand_to_fill_screen_around_block(self, &cursor_id).await?;

        // Scroll based on last cursor position
        let last_cursor_id = TreeBlockId::from_cursor(&self.context.last_cursor);
        if !renderer::scroll_to_set_block_top(self, &last_cursor_id, self.context.last_cursor_top) {
            // Since the last cursor is not within scrolling distance of our
            // current cursor, we need to estimate whether the last cursor was
            // above or below the current cursor.
            let last_cursor_root_id = self.root_id(&last_cursor_id).await?;
            if Self::root_id_is_above_root_id(last_cursor_root_id, cursor_root_id) {
                renderer::scroll_blocks_fully_below_screen(self);
            } else {
                renderer::scroll_blocks_fully_above_screen(self);
            }
        }

        // Fulfill scroll constraints
        self.make_cursor_visible();
        renderer::clamp_scroll_biased_downwards(self);

        Ok(())
    }

    fn move_cursor_so_it_is_visible(&mut self) {
        let cursor_id = TreeBlockId::from_cursor(self.cursor);
        if matches!(cursor_id, TreeBlockId::Bottom | TreeBlockId::Msg(_)) {
            match renderer::find_cursor_starting_at(self, &cursor_id) {
                Some(TreeBlockId::Bottom) => *self.cursor = Cursor::Bottom,
                Some(TreeBlockId::Msg(id)) => *self.cursor = Cursor::Msg(id.clone()),
                _ => {}
            }
        }
    }

    pub async fn scroll_by(&mut self, delta: i32) -> Result<(), S::Error> {
        self.blocks.shift(delta);
        renderer::expand_to_fill_visible_area(self).await?;
        renderer::clamp_scroll_biased_downwards(self);

        self.move_cursor_so_it_is_visible();

        self.make_cursor_visible();
        renderer::clamp_scroll_biased_downwards(self);

        Ok(())
    }

    pub fn center_cursor(&mut self) {
        let cursor_id = TreeBlockId::from_cursor(self.cursor);
        renderer::scroll_so_block_is_centered(self, &cursor_id);

        self.make_cursor_visible();
        renderer::clamp_scroll_biased_downwards(self);
    }

    pub fn update_render_info(
        &self,
        last_cursor: &mut Cursor<M::Id>,
        last_cursor_top: &mut i32,
        last_visible_msgs: &mut Vec<M::Id>,
    ) {
        *last_cursor = self.cursor.clone();

        let cursor_id = TreeBlockId::from_cursor(self.cursor);
        let (range, _) = self.blocks.find_block(&cursor_id).unwrap();
        *last_cursor_top = range.top;

        let area = renderer::visible_area(self);
        *last_visible_msgs = self
            .blocks
            .iter()
            .filter(|(range, _)| overlaps(area, *range))
            .filter_map(|(_, block)| block.id().msg_id())
            .cloned()
            .collect()
    }

    pub fn into_visible_blocks(
        self,
    ) -> impl Iterator<Item = (Range<i32>, Block<TreeBlockId<M::Id>>)> {
        let area = renderer::visible_area(&self);
        self.blocks
            .into_iter()
            .filter(move |(range, block)| overlaps(area, block.focus(*range)))
    }
}

#[async_trait]
impl<M, S> Renderer<TreeBlockId<M::Id>> for TreeRenderer<'_, M, S>
where
    M: Msg + ChatMsg + Send + Sync,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
    S::Error: Send,
{
    type Error = S::Error;

    fn size(&self) -> Size {
        self.context.size
    }

    fn scrolloff(&self) -> i32 {
        2 // TODO Make configurable
    }

    fn blocks(&self) -> &TreeBlocks<M::Id> {
        &self.blocks
    }

    fn blocks_mut(&mut self) -> &mut TreeBlocks<M::Id> {
        &mut self.blocks
    }

    fn into_blocks(self) -> TreeBlocks<M::Id> {
        self.blocks
    }

    async fn expand_top(&mut self) -> Result<(), Self::Error> {
        let prev_root_id = if let Some(top_root_id) = &self.top_root_id {
            self.store.prev_root_id(top_root_id).await?
        } else {
            self.store.last_root_id().await?
        };

        if let Some(prev_root_id) = prev_root_id {
            let tree = self.store.tree(&prev_root_id).await?;
            let blocks = self.layout_tree(tree);
            self.blocks.append_top(blocks);
            self.top_root_id = Some(prev_root_id);
        } else {
            self.blocks.end_top();
        }

        Ok(())
    }

    async fn expand_bottom(&mut self) -> Result<(), Self::Error> {
        let Some(bottom_root_id) = &self.bottom_root_id else {
            self.blocks.end_bottom();
            return Ok(());
        };

        let next_root_id = self.store.next_root_id(bottom_root_id).await?;
        if let Some(next_root_id) = next_root_id {
            let tree = self.store.tree(&next_root_id).await?;
            let blocks = self.layout_tree(tree);
            self.blocks.append_bottom(blocks);
            self.bottom_root_id = Some(next_root_id);
        } else {
            let blocks = self.layout_bottom();
            self.blocks.append_bottom(blocks);
            self.blocks.end_bottom();
            self.bottom_root_id = None;
        };

        Ok(())
    }
}
