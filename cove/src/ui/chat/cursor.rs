//! Common cursor movement logic.

use std::{collections::HashSet, hash::Hash};

use crate::store::{Msg, MsgStore, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor<Id> {
    Bottom,
    Msg(Id),
    Editor {
        coming_from: Option<Id>,
        parent: Option<Id>,
    },
    Pseudo {
        coming_from: Option<Id>,
        parent: Option<Id>,
    },
}

impl<Id: Clone + Eq + Hash> Cursor<Id> {
    fn find_parent<M>(tree: &Tree<M>, id: &mut Id) -> bool
    where
        M: Msg<Id = Id>,
    {
        if let Some(parent) = tree.parent(id) {
            *id = parent;
            true
        } else {
            false
        }
    }

    /// Move to the previous sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_prev_sibling<M, S>(
        store: &S,
        tree: &mut Tree<M>,
        id: &mut Id,
    ) -> Result<bool, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        let moved = if let Some(prev_sibling) = tree.prev_sibling(id) {
            *id = prev_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the previous tree.
            if let Some(prev_root_id) = store.prev_root_id(tree.root()).await? {
                *tree = store.tree(&prev_root_id).await?;
                *id = prev_root_id;
                true
            } else {
                false
            }
        } else {
            false
        };
        Ok(moved)
    }

    /// Move to the next sibling, or don't move if this is not possible.
    ///
    /// Always stays at the same level of indentation.
    async fn find_next_sibling<M, S>(
        store: &S,
        tree: &mut Tree<M>,
        id: &mut Id,
    ) -> Result<bool, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        let moved = if let Some(next_sibling) = tree.next_sibling(id) {
            *id = next_sibling;
            true
        } else if tree.parent(id).is_none() {
            // We're at the root of our tree, so we need to move to the root of
            // the next tree.
            if let Some(next_root_id) = store.next_root_id(tree.root()).await? {
                *tree = store.tree(&next_root_id).await?;
                *id = next_root_id;
                true
            } else {
                false
            }
        } else {
            false
        };
        Ok(moved)
    }

    fn find_first_child_in_tree<M>(folded: &HashSet<Id>, tree: &Tree<M>, id: &mut Id) -> bool
    where
        M: Msg<Id = Id>,
    {
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

    fn find_last_child_in_tree<M>(folded: &HashSet<Id>, tree: &Tree<M>, id: &mut Id) -> bool
    where
        M: Msg<Id = Id>,
    {
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

    /// Move to the message above, or don't move if this is not possible.
    async fn find_above_msg_in_tree<M, S>(
        store: &S,
        folded: &HashSet<Id>,
        tree: &mut Tree<M>,
        id: &mut Id,
    ) -> Result<bool, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        // Move to previous sibling, then to its last child
        // If not possible, move to parent
        let moved = if Self::find_prev_sibling(store, tree, id).await? {
            while Self::find_last_child_in_tree(folded, tree, id) {}
            true
        } else {
            Self::find_parent(tree, id)
        };
        Ok(moved)
    }

    /// Move to the next message, or don't move if this is not possible.
    async fn find_below_msg_in_tree<M, S>(
        store: &S,
        folded: &HashSet<Id>,
        tree: &mut Tree<M>,
        id: &mut Id,
    ) -> Result<bool, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        if Self::find_first_child_in_tree(folded, tree, id) {
            return Ok(true);
        }

        if Self::find_next_sibling(store, tree, id).await? {
            return Ok(true);
        }

        // Temporary id to avoid modifying the original one if no parent-sibling
        // can be found.
        let mut tmp_id = id.clone();
        while Self::find_parent(tree, &mut tmp_id) {
            if Self::find_next_sibling(store, tree, &mut tmp_id).await? {
                *id = tmp_id;
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn move_to_top<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        if let Some(first_root_id) = store.first_root_id().await? {
            *self = Self::Msg(first_root_id);
        }
        Ok(())
    }

    pub fn move_to_bottom(&mut self) {
        *self = Self::Bottom;
    }

    pub async fn move_to_older_msg<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(id) => {
                if let Some(prev_id) = store.older_msg_id(id).await? {
                    *id = prev_id;
                }
            }
            Self::Bottom | Self::Pseudo { .. } => {
                if let Some(id) = store.newest_msg_id().await? {
                    *self = Self::Msg(id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_newer_msg<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(id) => {
                if let Some(prev_id) = store.newer_msg_id(id).await? {
                    *id = prev_id;
                } else {
                    *self = Self::Bottom;
                }
            }
            Self::Pseudo { .. } => {
                *self = Self::Bottom;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_older_unseen_msg<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(id) => {
                if let Some(prev_id) = store.older_unseen_msg_id(id).await? {
                    *id = prev_id;
                }
            }
            Self::Bottom | Self::Pseudo { .. } => {
                if let Some(id) = store.newest_unseen_msg_id().await? {
                    *self = Self::Msg(id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_newer_unseen_msg<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(id) => {
                if let Some(prev_id) = store.newer_unseen_msg_id(id).await? {
                    *id = prev_id;
                } else {
                    *self = Self::Bottom;
                }
            }
            Self::Pseudo { .. } => {
                *self = Self::Bottom;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_parent<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Editor { parent, .. } | Self::Pseudo { parent, .. } => {
                if let Some(parent_id) = parent {
                    *self = Self::Msg(parent_id.clone())
                }
            }

            Self::Msg(id) => {
                let path = store.path(id).await?;
                if let Some(parent_id) = path.parent_segments().last() {
                    *id = parent_id.clone();
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_root<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let path = store.path(parent).await?;
                *self = Self::Msg(path.first().clone());
            }
            Self::Msg(id) => {
                let path = store.path(id).await?;
                *id = path.first().clone();
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_to_prev_sibling<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Bottom | Self::Pseudo { parent: None, .. } => {
                if let Some(last_root_id) = store.last_root_id().await? {
                    *self = Self::Msg(last_root_id);
                }
            }
            Self::Msg(msg) => {
                let path = store.path(msg).await?;
                let mut tree = store.tree(path.first()).await?;
                Self::find_prev_sibling(store, &mut tree, msg).await?;
            }
            Self::Editor { .. } => {}
            Self::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let path = store.path(parent).await?;
                let tree = store.tree(path.first()).await?;
                if let Some(children) = tree.children(parent) {
                    if let Some(last_child) = children.last() {
                        *self = Self::Msg(last_child.clone());
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn move_to_next_sibling<M, S>(&mut self, store: &S) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(msg) => {
                let path = store.path(msg).await?;
                let mut tree = store.tree(path.first()).await?;
                if !Self::find_next_sibling(store, &mut tree, msg).await?
                    && tree.parent(msg).is_none()
                {
                    *self = Self::Bottom;
                }
            }
            Self::Pseudo { parent: None, .. } => {
                *self = Self::Bottom;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn move_up_in_tree<M, S>(
        &mut self,
        store: &S,
        folded: &HashSet<Id>,
    ) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Bottom | Self::Pseudo { parent: None, .. } => {
                if let Some(last_root_id) = store.last_root_id().await? {
                    let tree = store.tree(&last_root_id).await?;
                    let mut id = last_root_id;
                    while Self::find_last_child_in_tree(folded, &tree, &mut id) {}
                    *self = Self::Msg(id);
                }
            }
            Self::Msg(msg) => {
                let path = store.path(msg).await?;
                let mut tree = store.tree(path.first()).await?;
                Self::find_above_msg_in_tree(store, folded, &mut tree, msg).await?;
            }
            Self::Editor { .. } => {}
            Self::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let tree = store.tree(parent).await?;
                let mut id = parent.clone();
                while Self::find_last_child_in_tree(folded, &tree, &mut id) {}
                *self = Self::Msg(id);
            }
        }
        Ok(())
    }

    pub async fn move_down_in_tree<M, S>(
        &mut self,
        store: &S,
        folded: &HashSet<Id>,
    ) -> Result<(), S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        match self {
            Self::Msg(msg) => {
                let path = store.path(msg).await?;
                let mut tree = store.tree(path.first()).await?;
                if !Self::find_below_msg_in_tree(store, folded, &mut tree, msg).await? {
                    *self = Self::Bottom;
                }
            }
            Self::Pseudo { parent: None, .. } => {
                *self = Self::Bottom;
            }
            Self::Pseudo {
                parent: Some(parent),
                ..
            } => {
                let mut tree = store.tree(parent).await?;
                let mut id = parent.clone();
                while Self::find_last_child_in_tree(folded, &tree, &mut id) {}
                // Now we're at the previous message
                if Self::find_below_msg_in_tree(store, folded, &mut tree, &mut id).await? {
                    *self = Self::Msg(id);
                } else {
                    *self = Self::Bottom;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The outer `Option` shows whether a parent exists or not. The inner
    /// `Option` shows if that parent has an id.
    pub async fn parent_for_normal_tree_reply<M, S>(
        &self,
        store: &S,
    ) -> Result<Option<Option<M::Id>>, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        Ok(match self {
            Self::Bottom => Some(None),
            Self::Msg(id) => {
                let path = store.path(id).await?;
                let tree = store.tree(path.first()).await?;

                Some(Some(if tree.next_sibling(id).is_some() {
                    // A reply to a message that has further siblings should be
                    // a direct reply. An indirect reply might end up a lot
                    // further down in the current conversation.
                    id.clone()
                } else if let Some(parent) = tree.parent(id) {
                    // A reply to a message without younger siblings should be
                    // an indirect reply so as not to create unnecessarily deep
                    // threads. In the case that our message has children, this
                    // might get a bit confusing. I'm not sure yet how well this
                    // "smart" reply actually works in practice.
                    parent
                } else {
                    // When replying to a top-level message, it makes sense to
                    // avoid creating unnecessary new threads.
                    id.clone()
                }))
            }
            _ => None,
        })
    }

    /// The outer `Option` shows whether a parent exists or not. The inner
    /// `Option` shows if that parent has an id.
    pub async fn parent_for_alternate_tree_reply<M, S>(
        &self,
        store: &S,
    ) -> Result<Option<Option<M::Id>>, S::Error>
    where
        M: Msg<Id = Id>,
        S: MsgStore<M>,
    {
        Ok(match self {
            Self::Bottom => Some(None),
            Self::Msg(id) => {
                let path = store.path(id).await?;
                let tree = store.tree(path.first()).await?;

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
        })
    }
}
