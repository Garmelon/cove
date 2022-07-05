mod action;
mod blocks;
mod cursor;
mod layout;
mod render;
mod util;

use std::marker::PhantomData;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use parking_lot::FairMutex;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

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

    pub async fn handle_navigation<S: MsgStore<M>>(
        &mut self,
        s: &mut S,
        c: &mut Option<Cursor<M::Id>>,
        t: &mut Terminal,
        z: Size,
        event: KeyEvent,
    ) {
        match event.code {
            KeyCode::Char('k') => self.move_up(s, c, t.frame(), z).await,
            KeyCode::Char('j') => self.move_down(s, c, t.frame(), z).await,
            KeyCode::Char('K') => self.move_up_sibling(s, c, t.frame(), z).await,
            KeyCode::Char('J') => self.move_down_sibling(s, c, t.frame(), z).await,
            KeyCode::Char('z') | KeyCode::Char('Z') => self.center_cursor(s, c, t.frame(), z).await,
            KeyCode::Char('g') => self.move_to_first(s, c, t.frame(), z).await,
            KeyCode::Char('G') => self.move_to_last(s, c, t.frame(), z).await,
            KeyCode::Esc => *c = None, // TODO Make 'G' do the same thing?
            _ => {}
        }
    }

    pub async fn handle_messaging<S: MsgStore<M>>(
        &mut self,
        s: &mut S,
        c: &mut Option<Cursor<M::Id>>,
        t: &mut Terminal,
        l: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> Option<(Option<M::Id>, String)> {
        match event.code {
            KeyCode::Char('r') => Self::reply_normal(s, c, t, l).await,
            KeyCode::Char('R') => Self::reply_alternate(s, c, t, l).await,
            KeyCode::Char('t') | KeyCode::Char('T') => Self::create_new_thread(t, l).await,
            _ => None,
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
        Self::render_blocks(frame, pos, size, blocks);
    }
}
