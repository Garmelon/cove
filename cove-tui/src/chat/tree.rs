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
        s: &mut S,
        c: &mut Option<Cursor<M::Id>>,
        f: &mut Frame,
        z: Size,
        event: KeyEvent,
    ) {
        match event.code {
            KeyCode::Char('z') | KeyCode::Char('Z') => self.center_cursor(s, c, f, z).await,
            KeyCode::Char('k') => self.move_up(s, c, f, z).await,
            KeyCode::Char('j') => self.move_down(s, c, f, z).await,
            KeyCode::Char('K') => self.move_up_sibling(s, c, f, z).await,
            KeyCode::Char('J') => self.move_down_sibling(s, c, f, z).await,
            KeyCode::Char('g') => self.move_to_first(s, c, f, z).await,
            KeyCode::Char('G') => self.move_to_last(s, c, f, z).await,
            KeyCode::Esc => *c = None, // TODO Make 'G' do the same thing?
            _ => {}
        }
    }

    pub async fn render<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        cursor: &Option<Cursor<M::Id>>,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        let blocks = self
            .layout_blocks(store, cursor.as_ref(), frame, size)
            .await;
        Self::render_blocks(frame, pos, size, &blocks);
    }
}
