use std::sync::Arc;

use parking_lot::FairMutex;
use toss::terminal::Terminal;

use crate::store::{Msg, MsgStore};
use crate::ui::util;

use super::{Cursor, InnerTreeViewState};

impl<M: Msg, S: MsgStore<M>> InnerTreeViewState<M, S> {
    pub async fn reply_normal(
        &self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> Option<(Option<M::Id>, String)> {
        match &self.cursor {
            Cursor::Bottom => {
                if let Some(content) = util::prompt(terminal, crossterm_lock) {
                    return Some((None, content));
                }
            }
            Cursor::Msg(msg) => {
                let path = self.store.path(msg).await;
                let tree = self.store.tree(path.first()).await;
                let parent_id = if tree.next_sibling(msg).is_some() {
                    // A reply to a message that has further siblings should be a
                    // direct reply. An indirect reply might end up a lot further
                    // down in the current conversation.
                    msg.clone()
                } else if let Some(parent) = tree.parent(msg) {
                    // A reply to a message without younger siblings should be
                    // an indirect reply so as not to create unnecessarily deep
                    // threads. In the case that our message has children, this
                    // might get a bit confusing. I'm not sure yet how well this
                    // "smart" reply actually works in practice.
                    parent
                } else {
                    // When replying to a top-level message, it makes sense to avoid
                    // creating unnecessary new threads.
                    msg.clone()
                };

                if let Some(content) = util::prompt(terminal, crossterm_lock) {
                    return Some((Some(parent_id), content));
                }
            }
            _ => {}
        }

        None
    }

    /// Does approximately the opposite of [`Self::reply_normal`].
    pub async fn reply_alternate(
        &self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> Option<(Option<M::Id>, String)> {
        match &self.cursor {
            Cursor::Bottom => {
                if let Some(content) = util::prompt(terminal, crossterm_lock) {
                    return Some((None, content));
                }
            }
            Cursor::Msg(msg) => {
                let path = self.store.path(msg).await;
                let tree = self.store.tree(path.first()).await;
                let parent_id = if tree.next_sibling(msg).is_none() {
                    // The opposite of replying normally
                    msg.clone()
                } else if let Some(parent) = tree.parent(msg) {
                    // The opposite of replying normally
                    parent
                } else {
                    // The same as replying normally, still to avoid creating
                    // unnecessary new threads
                    msg.clone()
                };

                if let Some(content) = util::prompt(terminal, crossterm_lock) {
                    return Some((Some(parent_id), content));
                }
            }
            _ => {}
        }

        None
    }

    pub fn create_new_thread(
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> Option<(Option<M::Id>, String)> {
        util::prompt(terminal, crossterm_lock).map(|content| (None, content))
    }
}
