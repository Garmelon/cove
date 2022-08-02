use std::iter;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::{FairMutex, Mutex};
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;
use toss::terminal::Terminal;
use unicode_segmentation::UnicodeSegmentation;

use crate::ui::util;

use super::Widget;

///////////
// State //
///////////

struct InnerEditorState {
    text: String,

    /// Index of the cursor in the text.
    ///
    /// Must point to a valid grapheme boundary.
    idx: usize,

    /// Width of the text when the editor was last rendered.
    ///
    /// Does not include additional column for cursor.
    last_width: u16,
}

impl InnerEditorState {
    fn new(text: String) -> Self {
        Self {
            idx: text.len(),
            last_width: 0,
            text,
        }
    }

    fn grapheme_boundaries(&self) -> Vec<usize> {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(iter::once(self.text.len()))
            .collect()
    }

    /// Ensure the cursor index lies on a grapheme boundary.
    ///
    /// If it doesn't, it is moved to the next grapheme boundary.
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

        // This loop should always return since the index behind the last
        // grapheme is included in the grapheme boundary iterator.
        panic!("cursor index out of bounds");
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
        self.idx = self.idx.min(self.text.len());
        self.move_cursor_to_grapheme_boundary();
    }

    /// Insert a character at the current cursor position and move the cursor
    /// accordingly.
    fn insert_char(&mut self, ch: char) {
        self.text.insert(self.idx, ch);
        self.idx += 1;
        self.move_cursor_to_grapheme_boundary();
    }

    /// Delete the grapheme before the cursor position.
    fn backspace(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *end == self.idx {
                self.text.replace_range(start..end, "");
                self.idx = *start;
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

    fn move_cursor_left(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *end == self.idx {
                self.idx = *start;
                break;
            }
        }
    }

    fn move_cursor_right(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                self.idx = *end;
                break;
            }
        }
    }

    /*
    fn wrap(&self, frame: &mut Frame, width: usize) -> Vec<Range<usize>> {
        let mut rows = vec![];
        let mut start = 0;
        let mut col = 0;
        for (i, g) in self.text.grapheme_indices(true) {
            let grapheme_width = if g == "\t" {
                frame.tab_width_at_column(col)
            } else {
                frame.grapheme_width(g)
            } as usize;

            if col + grapheme_width > width {
                rows.push(start..i);
                start = i;
                col = grapheme_width;
            } else {
                col += grapheme_width;
            }
        }
        rows.push(start..self.text.len());
        rows
    }

    pub fn render_highlighted<F>(&self, frame: &mut Frame, pos: Pos, width: usize, highlight: F)
    where
        F: Fn(&str) -> Styled,
    {
        let text = highlight(&self.text);
        let row_ranges = self.wrap(frame, width);
        let breakpoints = row_ranges
            .iter()
            .skip(1)
            .map(|r| r.start)
            .collect::<Vec<_>>();
        let rows = text.split_at_indices(&breakpoints);
        for (i, row) in rows.into_iter().enumerate() {
            let pos = pos + Pos::new(0, i as i32);
            frame.write(pos, row);
        }
    }

    pub fn render_with_style(
        &self,
        frame: &mut Frame,
        pos: Pos,
        width: usize,
        style: ContentStyle,
    ) {
        self.render_highlighted(frame, pos, width, |s| Styled::new((s, style)));
    }

    pub fn render(&self, frame: &mut Frame, pos: Pos, width: usize) {
        self.render_highlighted(frame, pos, width, |s| Styled::new(s));
    }
    */
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
        }
    }

    pub fn text(&self) -> String {
        self.0.lock().text.clone()
    }

    pub fn set_text(&self, text: String) {
        self.0.lock().set_text(text);
    }

    pub fn clear(&self) {
        self.set_text(String::new());
    }

    pub fn insert_char(&self, ch: char) {
        self.0.lock().insert_char(ch);
    }

    /// Delete the grapheme before the cursor position.
    pub fn backspace(&self) {
        self.0.lock().backspace();
    }

    /// Delete the grapheme after the cursor position.
    pub fn delete(&self) {
        self.0.lock().delete();
    }

    pub fn move_cursor_left(&self) {
        self.0.lock().move_cursor_left();
    }

    pub fn move_cursor_right(&self) {
        self.0.lock().move_cursor_right();
    }

    pub fn edit_externally(&self, terminal: &mut Terminal, crossterm_lock: &Arc<FairMutex<()>>) {
        let mut guard = self.0.lock();
        if let Some(text) = util::prompt(terminal, crossterm_lock, &guard.text) {
            guard.set_text(text);
        }
    }
}

////////////
// Widget //
////////////

pub struct Editor {
    state: Arc<Mutex<InnerEditorState>>,
    text: Styled,
    idx: usize,
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

    pub fn cursor_row(&self, frame: &mut Frame) -> usize {
        let width = self.state.lock().last_width;
        let text_width = (width - 1) as usize;
        let indices = frame.wrap(self.text.text(), text_width);
        let (row, _) = Self::wrapped_cursor(self.idx, &indices);
        row
    }
}

#[async_trait]
impl Widget for Editor {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let max_width = max_width.map(|w| w as usize).unwrap_or(usize::MAX).max(1);
        let max_text_width = max_width - 1;
        let indices = frame.wrap(self.text.text(), max_text_width);
        let lines = self.text.clone().split_at_indices(&indices);

        let min_width = lines
            .iter()
            .map(|l| frame.width(l.text().trim_end()))
            .max()
            .unwrap_or(0)
            + 1;
        let min_height = lines.len();
        Size::new(min_width as u16, min_height as u16)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let width = frame.size().width.max(1);
        let text_width = (width - 1) as usize;
        let indices = frame.wrap(self.text.text(), text_width);
        let lines = self.text.split_at_indices(&indices);

        let (cursor_row, cursor_line_idx) = Self::wrapped_cursor(self.idx, &indices);
        let cursor_col = frame.width(lines[cursor_row].text().split_at(cursor_line_idx).0);
        frame.set_cursor(Some(Pos::new(cursor_col as i32, cursor_row as i32)));

        for (i, line) in lines.into_iter().enumerate() {
            frame.write(Pos::new(0, i as i32), line);
        }

        self.state.lock().last_width = width;
    }
}
