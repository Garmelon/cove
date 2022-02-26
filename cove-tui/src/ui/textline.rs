use std::cmp;

use crossterm::event::{KeyCode, KeyEvent};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use super::input::EventHandler;

/// A simple single-line text box.
pub struct TextLine;

impl StatefulWidget for TextLine {
    type State = TextLineState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        Paragraph::new(&state.content as &str).render(area, buf);

        // Determine cursor position
        let prefix = state.content.chars().take(state.cursor).collect::<String>();
        let position = prefix.width() as u16;
        let x = area.x + position.min(area.width);
        state.last_cursor_pos = (x, area.y);
    }
}

/// State for [`TextLine`].
#[derive(Debug, Default)]
pub struct TextLineState {
    content: String,
    cursor: usize,
    last_cursor_pos: (u16, u16),
}

impl TextLineState {
    pub fn content(&self) -> String {
        self.content.clone()
    }

    /// The cursor's position from when the widget was last rendered.
    pub fn last_cursor_pos(&self) -> (u16, u16) {
        self.last_cursor_pos
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
