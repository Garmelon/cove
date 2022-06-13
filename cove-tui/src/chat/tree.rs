use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::traits::{Msg, MsgStore};

pub struct TreeView {}

impl TreeView {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle_key_event<M, S>(
        &mut self,
        store: &mut S,
        cursor: &mut Option<M::Id>,
        event: KeyEvent,
        size: Size,
    ) where
        M: Msg,
        S: MsgStore<M>,
    {
        // TODO
    }

    pub fn render<M: Msg, S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        // TODO
        frame.write(Pos::new(0, 0), "Hello world!", ContentStyle::default());
    }
}
