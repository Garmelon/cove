use std::sync::Arc;
use std::{io, iter};

use async_trait::async_trait;
use crossterm::style::Stylize;
use parking_lot::{FairMutex, Mutex};
use toss::{Frame, Pos, Size, Style, Styled, Terminal, WidthDb};
use unicode_segmentation::UnicodeSegmentation;

use crate::ui::util;

use super::text::Text;
use super::Widget;

/// Like [`WidthDb::wrap`] but includes a final break index if the text ends
/// with a newline.
fn wrap(widthdb: &mut WidthDb, text: &str, width: usize) -> Vec<usize> {
    let mut breaks = widthdb.wrap(text, width);
    if text.ends_with('\n') {
        breaks.push(text.len())
    }
    breaks
}

///////////
// State //
///////////

struct InnerEditorState {
    text: String,

    /// Index of the cursor in the text.
    ///
    /// Must point to a valid grapheme boundary.
    idx: usize,

    /// Column of the cursor on the screen just after it was last moved
    /// horizontally.
    col: usize,

    /// Width of the text when the editor was last rendered.
    ///
    /// Does not include additional column for cursor.
    last_width: u16,
}

impl InnerEditorState {
    fn new(text: String) -> Self {
        Self {
            idx: text.len(),
            col: 0,
            last_width: u16::MAX,
            text,
        }
    }

    ///////////////////////////////
    // Grapheme helper functions //
    ///////////////////////////////

    fn grapheme_boundaries(&self) -> Vec<usize> {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(iter::once(self.text.len()))
            .collect()
    }

    /// Ensure the cursor index lies on a grapheme boundary. If it doesn't, it
    /// is moved to the next grapheme boundary.
    ///
    /// Can handle arbitrary cursor index.
    fn move_cursor_to_grapheme_boundary(&mut self) {
        for i in self.grapheme_boundaries() {
            #[allow(clippy::comparison_chain)]
            if i == self.idx {
                // We're at a valid grapheme boundary already
                return;
            } else if i > self.idx {
                // There was no valid grapheme boundary at our cursor index, so
                // we'll take the next one we can get.
                self.idx = i;
                return;
            }
        }

        // The cursor was out of bounds, so move it to the last valid index.
        self.idx = self.text.len();
    }

    ///////////////////////////////
    // Line/col helper functions //
    ///////////////////////////////

    /// Like [`Self::grapheme_boundaries`] but for lines.
    ///
    /// Note that the last line can have a length of 0 if the text ends with a
    /// newline.
    fn line_boundaries(&self) -> Vec<usize> {
        let newlines = self
            .text
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i + 1); // utf-8 encodes '\n' as a single byte
        iter::once(0)
            .chain(newlines)
            .chain(iter::once(self.text.len()))
            .collect()
    }

    /// Find the cursor's current line.
    ///
    /// Returns `(line_nr, start_idx, end_idx)`.
    fn cursor_line(&self, boundaries: &[usize]) -> (usize, usize, usize) {
        let mut result = (0, 0, 0);
        for (i, (start, end)) in boundaries.iter().zip(boundaries.iter().skip(1)).enumerate() {
            if self.idx >= *start {
                result = (i, *start, *end);
            } else {
                break;
            }
        }
        result
    }

    fn cursor_col(&self, widthdb: &mut WidthDb, line_start: usize) -> usize {
        widthdb.width(&self.text[line_start..self.idx])
    }

    fn line(&self, line: usize) -> (usize, usize) {
        let boundaries = self.line_boundaries();
        boundaries
            .iter()
            .copied()
            .zip(boundaries.iter().copied().skip(1))
            .nth(line)
            .expect("line exists")
    }

    fn move_cursor_to_line_col(&mut self, widthdb: &mut WidthDb, line: usize, col: usize) {
        let (start, end) = self.line(line);
        let line = &self.text[start..end];

        let mut width = 0;
        for (gi, g) in line.grapheme_indices(true) {
            self.idx = start + gi;
            if col > width {
                width += widthdb.grapheme_width(g, width) as usize;
            } else {
                return;
            }
        }

        if !line.ends_with('\n') {
            self.idx = end;
        }
    }

    fn record_cursor_col(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.line_boundaries();
        let (_, start, _) = self.cursor_line(&boundaries);
        self.col = self.cursor_col(widthdb, start);
    }

    /////////////
    // Editing //
    /////////////

    fn clear(&mut self) {
        self.text = String::new();
        self.idx = 0;
        self.col = 0;
    }

    fn set_text(&mut self, widthdb: &mut WidthDb, text: String) {
        self.text = text;
        self.move_cursor_to_grapheme_boundary();
        self.record_cursor_col(widthdb);
    }

    /// Insert a character at the current cursor position and move the cursor
    /// accordingly.
    fn insert_char(&mut self, widthdb: &mut WidthDb, ch: char) {
        self.text.insert(self.idx, ch);
        self.idx += ch.len_utf8();
        self.record_cursor_col(widthdb);
    }

    /// Insert a string at the current cursor position and move the cursor
    /// accordingly.
    fn insert_str(&mut self, widthdb: &mut WidthDb, str: &str) {
        self.text.insert_str(self.idx, str);
        self.idx += str.len();
        self.record_cursor_col(widthdb);
    }

    /// Delete the grapheme before the cursor position.
    fn backspace(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *end == self.idx {
                self.text.replace_range(start..end, "");
                self.idx = *start;
                self.record_cursor_col(widthdb);
                break;
            }
        }
    }

    /// Delete the grapheme after the cursor position.
    fn delete(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                self.text.replace_range(start..end, "");
                break;
            }
        }
    }

    /////////////////////
    // Cursor movement //
    /////////////////////

    fn move_cursor_left(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *end == self.idx {
                self.idx = *start;
                self.record_cursor_col(widthdb);
                break;
            }
        }
    }

    fn move_cursor_right(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                self.idx = *end;
                self.record_cursor_col(widthdb);
                break;
            }
        }
    }

    fn move_cursor_left_a_word(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.grapheme_boundaries();
        let mut encountered_word = false;
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)).rev() {
            if *end == self.idx {
                let g = &self.text[*start..*end];
                let whitespace = g.chars().all(|c| c.is_whitespace());
                if encountered_word && whitespace {
                    break;
                } else if !whitespace {
                    encountered_word = true;
                }
                self.idx = *start;
            }
        }
        self.record_cursor_col(widthdb);
    }

    fn move_cursor_right_a_word(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.grapheme_boundaries();
        let mut encountered_word = false;
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                let g = &self.text[*start..*end];
                let whitespace = g.chars().all(|c| c.is_whitespace());
                if encountered_word && whitespace {
                    break;
                } else if !whitespace {
                    encountered_word = true;
                }
                self.idx = *end;
            }
        }
        self.record_cursor_col(widthdb);
    }

    fn move_cursor_to_start_of_line(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.line_boundaries();
        let (line, _, _) = self.cursor_line(&boundaries);
        self.move_cursor_to_line_col(widthdb, line, 0);
        self.record_cursor_col(widthdb);
    }

    fn move_cursor_to_end_of_line(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.line_boundaries();
        let (line, _, _) = self.cursor_line(&boundaries);
        self.move_cursor_to_line_col(widthdb, line, usize::MAX);
        self.record_cursor_col(widthdb);
    }

    fn move_cursor_up(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.line_boundaries();
        let (line, _, _) = self.cursor_line(&boundaries);
        if line > 0 {
            self.move_cursor_to_line_col(widthdb, line - 1, self.col);
        }
    }

    fn move_cursor_down(&mut self, widthdb: &mut WidthDb) {
        let boundaries = self.line_boundaries();

        // There's always at least one line, and always at least two line
        // boundaries at 0 and self.text.len().
        let amount_of_lines = boundaries.len() - 1;

        let (line, _, _) = self.cursor_line(&boundaries);
        if line + 1 < amount_of_lines {
            self.move_cursor_to_line_col(widthdb, line + 1, self.col);
        }
    }
}

pub struct EditorState(Arc<Mutex<InnerEditorState>>);

impl EditorState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InnerEditorState::new(String::new()))))
    }

    pub fn with_initial_text(text: String) -> Self {
        Self(Arc::new(Mutex::new(InnerEditorState::new(text))))
    }

    pub fn widget(&self) -> Editor {
        let guard = self.0.lock();
        let text = Styled::new_plain(guard.text.clone());
        let idx = guard.idx;
        Editor {
            state: self.0.clone(),
            text,
            idx,
            focus: true,
            hidden: None,
        }
    }

    pub fn text(&self) -> String {
        self.0.lock().text.clone()
    }

    pub fn clear(&self) {
        self.0.lock().clear();
    }

    pub fn set_text(&self, widthdb: &mut WidthDb, text: String) {
        self.0.lock().set_text(widthdb, text);
    }

    pub fn insert_char(&self, widthdb: &mut WidthDb, ch: char) {
        self.0.lock().insert_char(widthdb, ch);
    }

    pub fn insert_str(&self, widthdb: &mut WidthDb, str: &str) {
        self.0.lock().insert_str(widthdb, str);
    }

    /// Delete the grapheme before the cursor position.
    pub fn backspace(&self, widthdb: &mut WidthDb) {
        self.0.lock().backspace(widthdb);
    }

    /// Delete the grapheme after the cursor position.
    pub fn delete(&self) {
        self.0.lock().delete();
    }

    pub fn move_cursor_left(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_left(widthdb);
    }

    pub fn move_cursor_right(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_right(widthdb);
    }

    pub fn move_cursor_left_a_word(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_left_a_word(widthdb);
    }

    pub fn move_cursor_right_a_word(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_right_a_word(widthdb);
    }

    pub fn move_cursor_to_start_of_line(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_to_start_of_line(widthdb);
    }

    pub fn move_cursor_to_end_of_line(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_to_end_of_line(widthdb);
    }

    pub fn move_cursor_up(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_up(widthdb);
    }

    pub fn move_cursor_down(&self, widthdb: &mut WidthDb) {
        self.0.lock().move_cursor_down(widthdb);
    }

    pub fn edit_externally(
        &self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> io::Result<()> {
        let mut guard = self.0.lock();
        let text = util::prompt(terminal, crossterm_lock, &guard.text)?;

        if text.trim().is_empty() {
            // The user likely wanted to abort the edit and has deleted the
            // entire text (bar whitespace left over by some editors).
            return Ok(());
        }

        if let Some(text) = text.strip_suffix('\n') {
            // Some editors like vim add a trailing newline that would look out
            // of place in cove's editor. To intentionally add a trailing
            // newline, simply add two in-editor.
            guard.set_text(terminal.widthdb(), text.to_string());
        } else {
            guard.set_text(terminal.widthdb(), text);
        }

        Ok(())
    }
}

////////////
// Widget //
////////////

pub struct Editor {
    state: Arc<Mutex<InnerEditorState>>,
    text: Styled,
    idx: usize,
    focus: bool,
    hidden: Option<Box<Text>>,
}

impl Editor {
    pub fn highlight<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&str) -> Styled,
    {
        let new_text = f(self.text.text());
        assert_eq!(self.text.text(), new_text.text());
        self.text = new_text;
        self
    }

    pub fn focus(mut self, active: bool) -> Self {
        self.focus = active;
        self
    }

    pub fn hidden(self) -> Self {
        self.hidden_with_placeholder(("<hidden>", Style::new().grey().italic()))
    }

    pub fn hidden_with_placeholder<S: Into<Styled>>(mut self, placeholder: S) -> Self {
        self.hidden = Some(Box::new(Text::new(placeholder)));
        self
    }

    fn wrapped_cursor(cursor_idx: usize, break_indices: &[usize]) -> (usize, usize) {
        let mut row = 0;
        let mut line_idx = cursor_idx;

        for break_idx in break_indices {
            if cursor_idx < *break_idx {
                break;
            } else {
                row += 1;
                line_idx = cursor_idx - break_idx;
            }
        }

        (row, line_idx)
    }

    pub fn cursor_row(&self, widthdb: &mut WidthDb) -> usize {
        let width = self.state.lock().last_width;
        let text_width = (width - 1) as usize;
        let indices = wrap(widthdb, self.text.text(), text_width);
        let (row, _) = Self::wrapped_cursor(self.idx, &indices);
        row
    }
}

#[async_trait]
impl Widget for Editor {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        if let Some(placeholder) = &self.hidden {
            let mut size = placeholder.size(frame, max_width, max_height);

            // Cursor needs to fit regardless of focus
            size.width = size.width.max(1);
            size.height = size.height.max(1);

            return size;
        }

        let widthdb = frame.widthdb();

        let max_width = max_width.map(|w| w as usize).unwrap_or(usize::MAX).max(1);
        let max_text_width = max_width - 1;
        let indices = wrap(widthdb, self.text.text(), max_text_width);
        let lines = self.text.clone().split_at_indices(&indices);

        let min_width = lines
            .iter()
            .map(|l| widthdb.width(l.text().trim_end()))
            .max()
            .unwrap_or(0)
            + 1;
        let min_height = lines.len();
        Size::new(min_width as u16, min_height as u16)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        if let Some(placeholder) = self.hidden {
            if !self.text.text().is_empty() {
                placeholder.render(frame).await;
            }
            if self.focus {
                frame.set_cursor(Some(Pos::ZERO));
            }
            return;
        }

        let size = frame.size();
        let widthdb = frame.widthdb();

        let width = size.width.max(1);
        let text_width = (width - 1) as usize;
        let indices = wrap(widthdb, self.text.text(), text_width);
        let lines = self.text.split_at_indices(&indices);

        // Determine cursor position now while we still have the lines.
        let cursor_pos = if self.focus {
            let (cursor_row, cursor_line_idx) = Self::wrapped_cursor(self.idx, &indices);
            let cursor_col = widthdb.width(lines[cursor_row].text().split_at(cursor_line_idx).0);
            let cursor_col = cursor_col.min(text_width);
            Some(Pos::new(cursor_col as i32, cursor_row as i32))
        } else {
            None
        };

        for (i, line) in lines.into_iter().enumerate() {
            frame.write(Pos::new(0, i as i32), line);
        }

        if let Some(pos) = cursor_pos {
            frame.set_cursor(Some(pos));
        }

        self.state.lock().last_width = width;
    }
}
