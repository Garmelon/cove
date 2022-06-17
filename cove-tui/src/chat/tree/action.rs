use std::sync::Arc;

use parking_lot::FairMutex;
use toss::terminal::Terminal;

use crate::chat::{Cursor, Handled};
use crate::store::{Msg, MsgStore};

use super::TreeView;

impl<M: Msg> TreeView<M> {
    fn prompt_msg(crossterm_lock: &Arc<FairMutex<()>>, terminal: &mut Terminal) -> Option<String> {
        let content = {
            let _guard = crossterm_lock.lock();
            terminal.suspend().expect("could not suspend");
            let content = edit::edit("").expect("could not edit");
            terminal.unsuspend().expect("could not unsuspend");
            content
        };

        if content.trim().is_empty() {
            None
        } else {
            Some(content)
        }
    }

    pub async fn reply_normal<S: MsgStore<M>>(
        crossterm_lock: &Arc<FairMutex<()>>,
        store: &S,
        cursor: &Option<Cursor<M::Id>>,
        terminal: &mut Terminal,
    ) -> Handled<M::Id> {
        if let Some(cursor) = cursor {
            let tree = store.tree(store.path(&cursor.id).await.first()).await;
            let parent_id = if tree.next_sibling(&cursor.id).is_some() {
                // A reply to a message that has further siblings should be a
                // direct reply. An indirect reply might end up a lot further
                // down in the current conversation.
                cursor.id.clone()
            } else if let Some(parent) = tree.parent(&cursor.id) {
                // A reply to a message without further siblings should be an
                // indirect reply so as not to create unnecessarily deep
                // threads. In the case that our message has children, this
                // might get a bit confusing. I'm not sure yet how well this
                // "smart" reply actually works in practice.
                parent
            } else {
                // When replying to a top-level message, it makes sense to avoid
                // creating unnecessary new threads.
                cursor.id.clone()
            };

            if let Some(content) = Self::prompt_msg(crossterm_lock, terminal) {
                return Handled::NewMessage {
                    parent: Some(parent_id),
                    content,
                };
            }
        }

        Handled::Ok
    }

    /// Does approximately the opposite of [`Self::reply_normal`].
    pub async fn reply_alternate<S: MsgStore<M>>(
        crossterm_lock: &Arc<FairMutex<()>>,
        store: &S,
        cursor: &Option<Cursor<M::Id>>,
        terminal: &mut Terminal,
    ) -> Handled<M::Id> {
        if let Some(cursor) = cursor {
            let tree = store.tree(store.path(&cursor.id).await.first()).await;
            let parent_id = if tree.next_sibling(&cursor.id).is_none() {
                // The opposite of replying normally
                cursor.id.clone()
            } else if let Some(parent) = tree.parent(&cursor.id) {
                // The opposite of replying normally
                parent
            } else {
                // The same as replying normally, still to avoid creating
                // unnecessary new threads
                cursor.id.clone()
            };

            if let Some(content) = Self::prompt_msg(crossterm_lock, terminal) {
                return Handled::NewMessage {
                    parent: Some(parent_id),
                    content,
                };
            }
        }

        Handled::Ok
    }

    pub async fn create_new_thread(
        crossterm_lock: &Arc<FairMutex<()>>,
        terminal: &mut Terminal,
    ) -> Handled<M::Id> {
        if let Some(content) = Self::prompt_msg(crossterm_lock, terminal) {
            Handled::NewMessage {
                parent: None,
                content,
            }
        } else {
            Handled::Ok
        }
    }
}
