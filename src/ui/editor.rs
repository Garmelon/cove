use std::iter;
use std::ops::Range;

use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos};
use toss::styled::Styled;
use unicode_segmentation::UnicodeSegmentation;

pub struct Editor {
    text: String,

    /// Index of the cursor in the text.
    ///
    /// Must point to a valid grapheme boundary.
    idx: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            idx: 0,
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

    /// Insert a character at the current cursor position and move the cursor
    /// accordingly.
    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.idx, ch);
        self.idx += 1;
        self.move_cursor_to_grapheme_boundary();
    }

    /// Delete the grapheme before the cursor position.
    pub fn backspace(&mut self) {
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
    pub fn delete(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                self.text.replace_range(start..end, "");
                break;
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *end == self.idx {
                self.idx = *start;
                break;
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        let boundaries = self.grapheme_boundaries();
        for (start, end) in boundaries.iter().zip(boundaries.iter().skip(1)) {
            if *start == self.idx {
                self.idx = *end;
                break;
            }
        }
    }

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
}
