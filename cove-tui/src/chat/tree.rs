mod blocks;
mod cursor;
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
        let blocks = self
            .layout_blocks(room, store, cursor.as_ref(), frame, size)
            .await;
        Self::render_blocks(frame, pos, size, &blocks);
    }
}
