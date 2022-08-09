//! Moving the cursor around.

use std::collections::HashSet;

use crate::store::{Msg, MsgStore, Tree};

use super::{Correction, InnerTreeViewState};

#[derive(Debug, Clone, Copy)]
pub enum Cursor<I> {
    Bottom,
    Msg(I),
    Editor {
        coming_from: Option<I>,
        parent: Option<I>,
    },
    Pseudo {
        coming_from: Option<I>,
        parent: Option<I>,
    },
}

impl<I> Cursor<I> {
    pub fn editor(coming_from: Option<I>, parent: Option<I>) -> Self {
        Self::Editor {
            coming_from,
            parent,
        }
    }
}

impl<I: Eq> Cursor<I> {
    pub fn refers_to(&self, id: &I) -> bool {
        if let Self::Msg(own_id) = self {
            own_id == id
        } else {
            false
        }
    }

    pub fn refers_to_last_child_of(&self, id: &I) -> bool {
        if let Self::Editor {
            parent: Some(parent),
            ..
        }
        | Self::Pseudo {
            parent: Some(parent),
            ..
        } = self
        {
            parent == id
        } else {
            false
        }
    }
}

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    fn find_parent(tree: &Tree<M>, id: &mut M::Id) -> bool {
        if let Some(parent) = tree.parent(id) {
            *id = parent;
            true
        } else {
            false
        }
    }

    fn find_first_child(folded: &HashSet<M::Id>, tree: &Tree<M>, id: &mut M::Id) -> bool {
        if folded.contains(id) {
            return false;
        }

        if let Some(child) = tree.children(id).and_then(|c| c.first()) {
            *id = child.clone();
            true
        } else {
            false
        }
    }

    fn find_last_child(folded: &HashSet<M::Id>, tree: &Tree<M>, id: &mut M::Id) -> bool {
        if folded.contains(id) {
            return false;
        }

        if let Some(child) = tree.children(id).and_then(|c| c.last()) {
            *id = child.clone();
            true
        } else {
            false
        }
    }

    /// Move to the previous sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_prev_sibling(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        if let Some(prev_sibling) = tree.prev_sibling(id) {
            *id = prev_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_tree_id) = store.prev_tree_id(tree.root()).await {
                *tree = store.tree(&prev_tree_id).await;
                *id = prev_tree_id;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Move to the next sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_next_sibling(store: &S, tree: &mut Tree<M>, id: &mut M::Id) -> bool {
        if let Some(next_sibling) = tree.next_sibling(id) {
            *id = next_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the next tree.
            if let Some(next_tree_id) = store.next_tree_id(tree.root()).await {
                *tree = store.tree(&next_tree_id).await;
                *id = next_tree_id;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Move to the previous message, or don't move if this is not possible.
    async fn find_prev_msg(
        store: &S,
        folded: &HashSet<M::Id>,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
        // Move to previous sibling, then to its last child
        // If not possible, move to parent
        if Self::find_prev_sibling(store, tree, id).await {
            while Self::find_last_child(folded, tree, id) {}
            true
        } else {
            Self::find_parent(tree, id)
        }
    }

    /// Move to the next message, or don't move if this is not possible.
    async fn find_next_msg(
        store: &S,
        folded: &HashSet<M::Id>,
        tree: &mut Tree<M>,
        id: &mut M::Id,
    ) -> bool {
        if Self::find_first_child(folded, tree, id) {
            return true;
        }

        if Self::find_next_sibling(store, tree, id).await {
            return true;
        }

        // Temporary id to avoid modifying the original one if no parent-sibling
        // can be found.
        let mut tmp_id = id.clone();
        while Self::find_parent(tree, &mut tmp_id) {
            if Self::find_next_sibling(store, tree, &mut tmp_id).await {
                *id = tmp_id;
                return true;
            }
        }

        false
    }

    pub async fn move_cursor_up(&mut self) {
        match &mut self.cursor {
            Cursor::Bottom | Cursor::Pseudo { parent: None, .. } => {
                if let Some(last_tree_id) = self.store.last_tree_id().await {
                    let tree = self.store.tree(&last_tree_id).await;
                    let mut id = last_tree_id;
                    while Self::find_last_child(&self.folded, &tree, &mut id) {}
                    self.cursor = Cursor::Msg(id);
                }
            }
            Cursor::Msg(ref mut msg) => {
                let path = self.store.path(msg).await;
                let mut tree = self.store.tree(path.first()).await;
                Self::find_prev_msg(&self.store, &self.folded, &mut tree, msg).await;
            }
            Cursor::Editor { .. } => {}
            Cursor::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let tree = self.store.tree(parent).await;
                let mut id = parent.clone();
                while Self::find_last_child(&self.folded, &tree, &mut id) {}
                self.cursor = Cursor::Msg(id);
            }
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_down(&mut self) {
        match &mut self.cursor {
            Cursor::Msg(ref mut msg) => {
                let path = self.store.path(msg).await;
                let mut tree = self.store.tree(path.first()).await;
                if !Self::find_next_msg(&self.store, &self.folded, &mut tree, msg).await {
                    self.cursor = Cursor::Bottom;
                }
            }
            Cursor::Pseudo { parent: None, .. } => {
                self.cursor = Cursor::Bottom;
            }
            Cursor::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let mut tree = self.store.tree(parent).await;
                let mut id = parent.clone();
                while Self::find_last_child(&self.folded, &tree, &mut id) {}
                // Now we're at the previous message
                if Self::find_next_msg(&self.store, &self.folded, &mut tree, &mut id).await {
                    self.cursor = Cursor::Msg(id);
                } else {
                    self.cursor = Cursor::Bottom;
                }
            }
            _ => {}
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_older(&mut self) {
        match &mut self.cursor {
            Cursor::Msg(id) => {
                if let Some(prev_id) = self.store.older_msg_id(id).await {
                    *id = prev_id;
                }
            }
            Cursor::Bottom | Cursor::Pseudo { .. } => {
                if let Some(id) = self.store.newest_msg_id().await {
                    self.cursor = Cursor::Msg(id);
                }
            }
            _ => {}
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_newer(&mut self) {
        match &mut self.cursor {
            Cursor::Msg(id) => {
                if let Some(prev_id) = self.store.newer_msg_id(id).await {
                    *id = prev_id;
                } else {
                    self.cursor = Cursor::Bottom;
                }
            }
            Cursor::Pseudo { .. } => {
                self.cursor = Cursor::Bottom;
            }
            _ => {}
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_older_unseen(&mut self) {
        match &mut self.cursor {
            Cursor::Msg(id) => {
                if let Some(prev_id) = self.store.older_unseen_msg_id(id).await {
                    *id = prev_id;
                }
            }
            Cursor::Bottom | Cursor::Pseudo { .. } => {
                if let Some(id) = self.store.newest_unseen_msg_id().await {
                    self.cursor = Cursor::Msg(id);
                }
            }
            _ => {}
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_newer_unseen(&mut self) {
        match &mut self.cursor {
            Cursor::Msg(id) => {
                if let Some(prev_id) = self.store.newer_unseen_msg_id(id).await {
                    *id = prev_id;
                } else {
                    self.cursor = Cursor::Bottom;
                }
            }
            Cursor::Pseudo { .. } => {
                self.cursor = Cursor::Bottom;
            }
            _ => {}
        }
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub async fn move_cursor_to_top(&mut self) {
        if let Some(first_tree_id) = self.store.first_tree_id().await {
            self.cursor = Cursor::Msg(first_tree_id);
            self.correction = Some(Correction::MakeCursorVisible);
        }
    }

    pub async fn move_cursor_to_bottom(&mut self) {
        self.cursor = Cursor::Bottom;
        // Not really necessary; only here for consistency with other methods
        self.correction = Some(Correction::MakeCursorVisible);
    }

    pub fn scroll_up(&mut self, amount: i32) {
        self.scroll += amount;
        self.correction = Some(Correction::MoveCursorToVisibleArea);
    }

    pub fn scroll_down(&mut self, amount: i32) {
        self.scroll -= amount;
        self.correction = Some(Correction::MoveCursorToVisibleArea);
    }

    pub async fn parent_for_normal_reply(&self) -> Option<Option<M::Id>> {
        match &self.cursor {
            Cursor::Bottom => Some(None),
            Cursor::Msg(id) => {
                let path = self.store.path(id).await;
                let tree = self.store.tree(path.first()).await;

                Some(Some(if tree.next_sibling(id).is_some() {
                    // A reply to a message that has further siblings should be a
                    // direct reply. An indirect reply might end up a lot further
                    // down in the current conversation.
                    id.clone()
                } else if let Some(parent) = tree.parent(id) {
                    // A reply to a message without younger siblings should be
                    // an indirect reply so as not to create unnecessarily deep
                    // threads. In the case that our message has children, this
                    // might get a bit confusing. I'm not sure yet how well this
                    // "smart" reply actually works in practice.
                    parent
                } else {
                    // When replying to a top-level message, it makes sense to avoid
                    // creating unnecessary new threads.
                    id.clone()
                }))
            }
            _ => None,
        }
    }

    pub async fn parent_for_alternate_reply(&self) -> Option<Option<M::Id>> {
        match &self.cursor {
            Cursor::Bottom => Some(None),
            Cursor::Msg(id) => {
                let path = self.store.path(id).await;
                let tree = self.store.tree(path.first()).await;

                Some(Some(if tree.next_sibling(id).is_none() {
                    // The opposite of replying normally
                    id.clone()
                } else if let Some(parent) = tree.parent(id) {
                    // The opposite of replying normally
                    parent
                } else {
                    // The same as replying normally, still to avoid creating
                    // unnecessary new threads
                    id.clone()
                }))
            }
            _ => None,
        }
    }
}

/*
    pub async fn move_up_sibling<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let path = store.path(&cursor.id).await;
            let mut tree = store.tree(path.first()).await;
            self.find_prev_sibling(store, &mut tree, &mut cursor.id)
                .await;
        } else if let Some(last_tree) = store.last_tree().await {
            // I think moving to the root of the last tree makes the most sense
            // here. Alternatively, we could just not move the cursor, but that
            // wouldn't be very useful.
            *cursor = Some(Cursor::new(last_tree));
        }
        // If neither condition holds, we can't set a cursor because there's no
        // message to move to.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    pub async fn move_down_sibling<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        let old_blocks = self
            .layout_blocks(store, cursor.as_ref(), frame, size)
            .await;
        let old_cursor_id = cursor.as_ref().map(|c| c.id.clone());

        if let Some(cursor) = cursor {
            let path = store.path(&cursor.id).await;
            let mut tree = store.tree(path.first()).await;
            self.find_next_sibling(store, &mut tree, &mut cursor.id)
                .await;
        }
        // If that condition doesn't hold, we're already at the bottom in
        // cursor-less mode and can't move further down anyways.

        if let Some(cursor) = cursor {
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }

    // TODO move_older[_unseen]
    // TODO move_newer[_unseen]

    pub async fn center_cursor<S: MsgStore<M>>(
        &mut self,
        store: &S,
        cursor: &mut Option<Cursor<M::Id>>,
        frame: &mut Frame,
        size: Size,
    ) {
        if let Some(cursor) = cursor {
            cursor.proportion = 0.5;

            // Correcting the offset just to make sure that this function
            // behaves nicely if the cursor has too many lines.
            let old_blocks = self.layout_blocks(store, Some(cursor), frame, size).await;
            let old_cursor_id = Some(cursor.id.clone());
            self.correct_cursor_offset(store, frame, size, &old_blocks, &old_cursor_id, cursor)
                .await;
        }
    }
*/
