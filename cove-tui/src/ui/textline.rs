use std::cmp;

use crossterm::event::{KeyCode, KeyEvent};
use tui::backend::Backend;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, StatefulWidget, Widget};
use tui::Frame;
use unicode_width::UnicodeWidthStr;

use super::input::EventHandler;

/// A simple single-line text box.
pub struct TextLine;

impl StatefulWidget for TextLine {
    type State = TextLineState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        Paragraph::new(&state.content as &str).render(area, buf);
        // Paragraph::new("foo").render(area, buf);
    }
}

/// State for [`TextLine`].
#[derive(Debug, Default)]
pub struct TextLineState {
    content: String,
    cursor: usize,
}

impl TextLineState {
    pub fn content(&self) -> String {
        self.content.clone()
    }

    /// Set a frame's cursor position to this text line's cursor position
    pub fn set_cursor<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let prefix = self.content.chars().take(self.cursor).collect::<String>();
        let position = prefix.width() as u16;
        let x = area.x + cmp::min(position, area.width);
        f.set_cursor(x, area.y);
    }

    fn chars(&self) -> usize {
        self.content.chars().count()
    }

    fn move_cursor_start(&mut self) {
        self.cursor = 0;
    }

    fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        self.cursor = cmp::min(self.cursor + 1, self.chars());
    }

    fn move_cursor_end(&mut self) {
        self.cursor = self.chars();
    }

    fn cursor_byte_offset(&self) -> usize {
        self.content
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.content.len())
    }
}

pub enum TextLineReaction {
    Handled,
    Close,
}

impl EventHandler for TextLineState {
    type Reaction = TextLineReaction;

    fn handle_key(&mut self, event: KeyEvent) -> Option<Self::Reaction> {
        match event.code {
            KeyCode::Backspace if self.cursor > 0 => {
                self.move_cursor_left();
                self.content.remove(self.cursor_byte_offset());
                Some(TextLineReaction::Handled)
            }
            KeyCode::Left => {
                self.move_cursor_left();
                Some(TextLineReaction::Handled)
            }
            KeyCode::Right => {
                self.move_cursor_right();
                Some(TextLineReaction::Handled)
            }
            KeyCode::Home => {
                self.move_cursor_start();
                Some(TextLineReaction::Handled)
            }
            KeyCode::End => {
                self.move_cursor_end();
                Some(TextLineReaction::Handled)
            }
            KeyCode::Delete if self.cursor < self.chars() => {
                self.content.remove(self.cursor_byte_offset());
                Some(TextLineReaction::Handled)
            }
            KeyCode::Char(c) => {
                self.content.insert(self.cursor_byte_offset(), c);
                self.move_cursor_right();
                Some(TextLineReaction::Handled)
            }
            KeyCode::Esc => Some(TextLineReaction::Close),
            _ => None,
        }
    }
}
