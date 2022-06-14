mod blocks;
mod layout;
mod render;
mod util;

use std::marker::PhantomData;

use crossterm::event::{KeyCode, KeyEvent};
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore};

use super::Cursor;

pub struct TreeView<M: Msg> {
    // pub focus: Option<M::Id>,
    // pub folded: HashSet<M::Id>,
    // pub minimized: HashSet<M::Id>,
    phantom: PhantomData<M::Id>, // TODO Remove
}

impl<M: Msg> TreeView<M> {
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }

    async fn move_to_prev_msg<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
    ) {
        let tree = if let Some(cursor) = cursor {
            let path = store.path(room, &cursor.id).await;
            let tree = store.tree(room, path.first()).await;
            if let Some(prev_sibling) = tree.prev_sibling(&cursor.id) {
                cursor.id = tree.last_child(prev_sibling.clone());
                return;
            } else if let Some(parent) = tree.parent(&cursor.id) {
                cursor.id = parent;
                return;
            } else {
                store.prev_tree(room, path.first()).await
            }
        } else {
            store.last_tree(room).await
        };

        if let Some(tree) = tree {
            let tree = store.tree(room, &tree).await;
            let cursor_id = tree.last_child(tree.root().clone());
            if let Some(cursor) = cursor {
                cursor.id = cursor_id;
            } else {
                *cursor = Some(Cursor {
                    id: cursor_id,
                    proportion: 1.0,
                });
            }
        }
    }

    async fn center_cursor(&mut self, cursor: &mut Option<Cursor<M::Id>>) {
        if let Some(cursor) = cursor {
            cursor.proportion = 0.5;
        }
    }

    pub async fn handle_key_event<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
        event: KeyEvent,
        frame: &mut Frame,
        size: Size,
    ) {
        match event.code {
            KeyCode::Char('k') => self.move_to_prev_msg(store, room, cursor).await,
            KeyCode::Char('z') => self.center_cursor(cursor).await,
            _ => {}
        }
    }

    pub async fn render<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        let blocks = self.layout_blocks(room, store, cursor, frame, size).await;
        Self::render_blocks(frame, pos, size, &blocks);
    }
}
