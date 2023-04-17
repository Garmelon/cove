use std::vec;

use toss::{Frame, Pos, Size, Widget, WidthDb};

#[derive(Debug, Clone)]
struct Cursor<Id> {
    /// Id of the element the cursor is pointing to.
    ///
    /// If the rows change (e.g. reorder) but there is still a row with this id,
    /// the cursor is moved to this row.
    id: Id,

    /// Index of the row the cursor is pointing to.
    ///
    /// If the rows change and there is no longer a row with the cursor's id,
    /// the cursor is moved up or down to the next selectable row. This way, it
    /// stays close to its previous position.
    idx: usize,
}

impl<Id> Cursor<Id> {
    pub fn new(id: Id, idx: usize) -> Self {
        Self { id, idx }
    }
}

#[derive(Debug)]
pub struct ListState<Id> {
    /// Amount of lines that the list is scrolled, i.e. offset from the top.
    offset: usize,

    /// A cursor within the list.
    ///
    /// Set to `None` if the list contains no selectable rows.
    cursor: Option<Cursor<Id>>,

    /// Height of the list when it was last rendered.
    last_height: u16,

    /// Rows when the list was last rendered.
    last_rows: Vec<Option<Id>>,
}

impl<Id> ListState<Id> {
    pub fn new() -> Self {
        Self {
            offset: 0,
            cursor: None,
            last_height: 0,
            last_rows: vec![],
        }
    }

    pub fn selected(&self) -> Option<&Id> {
        self.cursor.as_ref().map(|cursor| &cursor.id)
    }
}

impl<Id: Clone> ListState<Id> {
    fn first_selectable(&self) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn last_selectable(&self) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn selectable_at_or_before_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .take(i + 1)
            .rev()
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn selectable_at_or_after_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .skip(i)
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn selectable_before_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .take(i)
            .rev()
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn selectable_after_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .skip(i + 1)
            .find_map(|(i, row)| row.as_ref().map(|id| Cursor::new(id.clone(), i)))
    }

    fn move_cursor_to_make_it_visible(&mut self) {
        if let Some(cursor) = &self.cursor {
            let first_visible_line_idx = self.offset;
            let last_visible_line_idx = self
                .offset
                .saturating_add(self.last_height.into())
                .saturating_sub(1);

            let new_cursor = if cursor.idx < first_visible_line_idx {
                self.selectable_at_or_after_index(first_visible_line_idx)
            } else if cursor.idx > last_visible_line_idx {
                self.selectable_at_or_before_index(last_visible_line_idx)
            } else {
                return;
            };

            if let Some(new_cursor) = new_cursor {
                self.cursor = Some(new_cursor);
            }
        }
    }

    fn scroll_so_cursor_is_visible(&mut self) {
        if self.last_height == 0 {
            // Cursor can't be visible because nothing is visible
            return;
        }

        if let Some(cursor) = &self.cursor {
            // As long as height > 0, min <= max is true
            let min = (cursor.idx + 1).saturating_sub(self.last_height.into());
            let max = cursor.idx; // Rows have a height of 1
            self.offset = self.offset.clamp(min, max);
        }
    }

    fn clamp_scrolling(&mut self) {
        let min = 0;
        let max = self.last_rows.len().saturating_sub(self.last_height.into());
        self.offset = self.offset.clamp(min, max);
    }

    fn scroll_to(&mut self, new_offset: usize) {
        self.offset = new_offset;
        self.clamp_scrolling();
        self.move_cursor_to_make_it_visible();
    }

    fn move_cursor_to(&mut self, new_cursor: Cursor<Id>) {
        self.cursor = Some(new_cursor);
        self.scroll_so_cursor_is_visible();
        self.clamp_scrolling();
    }

    /// Scroll the list up by an amount of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_to(self.offset.saturating_sub(lines));
    }

    /// Scroll the list down by an amount of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_to(self.offset.saturating_add(lines));
    }

    /// Scroll so that the cursor is in the center of the widget, or at least as
    /// close as possible.
    pub fn center_cursor(&mut self) {
        if let Some(cursor) = &self.cursor {
            let height: usize = self.last_height.into();
            self.scroll_to(cursor.idx.saturating_sub(height / 2));
        }
    }

    /// Move the cursor up to the next selectable row.
    pub fn move_cursor_up(&mut self) {
        if let Some(cursor) = &self.cursor {
            if let Some(new_cursor) = self.selectable_before_index(cursor.idx) {
                self.move_cursor_to(new_cursor);
            }
        }
    }

    /// Move the cursor down to the next selectable row.
    pub fn move_cursor_down(&mut self) {
        if let Some(cursor) = &self.cursor {
            if let Some(new_cursor) = self.selectable_after_index(cursor.idx) {
                self.move_cursor_to(new_cursor);
            }
        }
    }

    /// Move the cursor to the first selectable row.
    pub fn move_cursor_to_top(&mut self) {
        if let Some(new_cursor) = self.first_selectable() {
            self.move_cursor_to(new_cursor);
        }
    }

    /// Move the cursor to the last selectable row.
    pub fn move_cursor_to_bottom(&mut self) {
        if let Some(new_cursor) = self.last_selectable() {
            self.move_cursor_to(new_cursor);
        }
    }
}

impl<Id: Clone + Eq> ListState<Id> {
    fn selectable_of_id(&self, id: &Id) -> Option<Cursor<Id>> {
        self.last_rows
            .iter()
            .enumerate()
            .find_map(|(i, row)| match row {
                Some(rid) if rid == id => Some(Cursor::new(rid.clone(), i)),
                _ => None,
            })
    }

    fn fix_cursor(&mut self) {
        let new_cursor = if let Some(cursor) = &self.cursor {
            self.selectable_of_id(&cursor.id)
                .or_else(|| self.selectable_at_or_before_index(cursor.idx))
                .or_else(|| self.selectable_at_or_after_index(cursor.idx))
        } else {
            self.first_selectable()
        };

        if let Some(new_cursor) = new_cursor {
            self.move_cursor_to(new_cursor);
        } else {
            self.cursor = None;
        }
    }
}

struct UnrenderedRow<'a, Id, W> {
    id: Option<Id>,
    widget: Box<dyn FnOnce(bool) -> W + 'a>,
}

pub struct ListBuilder<'a, Id, W> {
    rows: Vec<UnrenderedRow<'a, Id, W>>,
}

impl<'a, Id, W> ListBuilder<'a, Id, W> {
    pub fn new() -> Self {
        Self { rows: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn add_unsel(&mut self, widget: W)
    where
        W: 'a,
    {
        self.rows.push(UnrenderedRow {
            id: None,
            widget: Box::new(|_| widget),
        });
    }

    pub fn add_sel(&mut self, id: Id, widget: impl FnOnce(bool) -> W + 'a) {
        self.rows.push(UnrenderedRow {
            id: Some(id),
            widget: Box::new(widget),
        });
    }

    pub fn build(self, state: &mut ListState<Id>) -> List<'_, Id, W>
    where
        Id: Clone + Eq,
    {
        state.last_rows = self.rows.iter().map(|row| row.id.clone()).collect();
        state.fix_cursor();

        let selected = state.selected();
        let rows = self
            .rows
            .into_iter()
            .map(|row| (row.widget)(row.id.as_ref() == selected))
            .collect();
        List { state, rows }
    }
}

pub struct List<'a, Id, W> {
    state: &'a mut ListState<Id>,
    rows: Vec<W>,
}

impl<Id, E, W> Widget<E> for List<'_, Id, W>
where
    Id: Clone + Eq,
    W: Widget<E>,
{
    fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        _max_height: Option<u16>,
    ) -> Result<Size, E> {
        let mut width = 0;
        for row in &self.rows {
            let size = row.size(widthdb, max_width, Some(1))?;
            width = width.max(size.width);
        }
        let height = self.rows.len().try_into().unwrap_or(u16::MAX);
        Ok(Size::new(width, height))
    }

    fn draw(self, frame: &mut Frame) -> Result<(), E> {
        let size = frame.size();

        self.state.last_height = size.height;

        for (y, row) in self
            .rows
            .into_iter()
            .skip(self.state.offset)
            .take(size.height.into())
            .enumerate()
        {
            frame.push(Pos::new(0, y as i32), Size::new(size.width, 1));
            row.draw(frame)?;
            frame.pop();
        }

        Ok(())
    }
}
