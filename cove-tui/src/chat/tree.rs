use std::collections::VecDeque;
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};

use crate::store::{Msg, MsgStore};

use super::Cursor;

struct Block<I> {
    line: i32,
    height: i32,
    id: Option<I>,
    cursor: bool,
    content: BlockContent,
}

enum BlockContent {
    Msg(MsgBlock),
    Placeholder,
}

struct MsgBlock {
    time: DateTime<Utc>,
    indent: usize,
    nick: String,
    content: Vec<String>,
}

struct Layout<I>(VecDeque<Block<I>>);

impl<I: PartialEq> Layout<I> {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    fn mark_cursor(&mut self, id: &I) {
        for block in &mut self.0 {
            if block.id.as_ref() == Some(id) {
                block.cursor = true;
            }
        }
    }

    fn calculate_offsets_with_cursor(&mut self, line: i32) {
        let cursor_index = self
            .0
            .iter()
            .enumerate()
            .find(|(_, b)| b.cursor)
            .expect("layout must contain cursor block")
            .0;

        // Propagate lines from cursor to both ends
        self.0[cursor_index].line = line;
        for i in (0..cursor_index).rev() {
            // let succ_line = self.0[i + 1].line;
            // let curr = &mut self.0[i];
            // curr.line = succ_line - curr.height;
            self.0[i].line = self.0[i + 1].line - self.0[i].height;
        }
        for i in (cursor_index + 1)..self.0.len() {
            // let pred = &self.0[i - 1];
            // self.0[i].line = pred.line + pred.height;
            self.0[i].line = self.0[i - 1].line + self.0[i - 1].height;
        }
    }

    fn calculate_offsets_without_cursor(&mut self, height: i32) {
        if let Some(back) = self.0.back_mut() {
            back.line = height - back.height;
        }
        for i in (0..self.0.len() - 1).rev() {
            self.0[i].line = self.0[i + 1].line - self.0[i].height;
        }
    }

    pub fn calculate_offsets(&mut self, height: i32, cursor: Option<Cursor<I>>) {
        if let Some(cursor) = cursor {
            let line = ((height - 1) as f32 * cursor.proportion) as i32;
            self.mark_cursor(&cursor.id);
            self.calculate_offsets_with_cursor(line);
        } else {
            self.calculate_offsets_without_cursor(height);
        }
    }
}

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

    async fn layout<S: MsgStore<M>>(
        &mut self,
        room: &str,
        store: S,
        cursor: &mut Option<Cursor<M::Id>>,
    ) -> Layout<M::Id> {
        if let Some(cursor) = cursor {
            // TODO Ensure focus lies on cursor path, otherwise unfocus
            // TODO Unfold all messages on path to cursor
            let cursor_path = store.path(room, &cursor.id).await;
            // TODO Produce layout of cursor subtree (with correct offsets)
            // TODO Expand layout upwards and downwards if there is no focus
            todo!()
        } else {
            // TODO Ensure there is no focus
            // TODO Produce layout of last tree (with correct offsets)
            // TODO Expand layout upwards
            todo!()
        }
    }

    pub fn handle_key_event<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        cursor: &mut Option<Cursor<M::Id>>,
        event: KeyEvent,
        size: Size,
    ) {
        // TODO
    }

    pub fn render<S: MsgStore<M>>(
        &mut self,
        store: &mut S,
        room: &str,
        frame: &mut Frame,
        pos: Pos,
        size: Size,
    ) {
        // TODO
        frame.write(Pos::new(0, 0), "Hello world!", ContentStyle::default());
    }
}
