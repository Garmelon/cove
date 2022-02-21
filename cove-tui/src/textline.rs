use std::cmp;

use crossterm::event::{Event, KeyCode};
use tui::backend::Backend;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, StatefulWidget, Widget};
use tui::Frame;
use unicode_width::UnicodeWidthStr;

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
}

impl TextLineState {
    pub fn set_cursor<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let x = area.x + (self.content.width() as u16);
        let x = cmp::min(x, area.x + area.width);
        f.set_cursor(x, area.y);
    }

    pub fn process_input(&mut self, event: Event) {
        if let Event::Key(k) = event {
            match k.code {
                KeyCode::Backspace => {
                    self.content.pop();
                }
                KeyCode::Char(c) => {
                    self.content.push(c);
                }
                _ => {}
            }
        }
    }
}
