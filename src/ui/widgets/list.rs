use std::sync::Arc;

use async_trait::async_trait;
use crossterm::style::ContentStyle;
use parking_lot::Mutex;
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;

use super::Widget;

///////////
// State //
///////////

#[derive(Debug)]
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
struct InnerListState<Id> {
    rows: Vec<Option<Id>>,
    offset: usize,
    cursor: Option<Cursor<Id>>,
    make_cursor_visible: bool,
}

impl<Id> InnerListState<Id> {
    fn new() -> Self {
        Self {
            rows: vec![],
            offset: 0,
            cursor: None,
            make_cursor_visible: false,
        }
    }
}

impl<Id: Clone> InnerListState<Id> {
    fn first_selectable(&self) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn last_selectable(&self) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn selectable_at_or_before_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .take(i + 1)
            .rev()
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn selectable_at_or_after_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .skip(i)
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn selectable_before_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .take(i)
            .rev()
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn selectable_after_index(&self, i: usize) -> Option<Cursor<Id>> {
        self.rows
            .iter()
            .enumerate()
            .skip(i + 1)
            .find_map(|(i, r)| r.as_ref().map(|c| Cursor::new(c.clone(), i)))
    }

    fn make_cursor_visible(&mut self, height: usize) {
        if height == 0 {
            // Cursor can't be visible because nothing is visible
            return;
        }

        if let Some(cursor) = &self.cursor {
            // As long as height > 0, min <= max is true
            let min = (cursor.idx + 1).saturating_sub(height);
            let max = cursor.idx;
            self.offset = self.offset.clamp(min, max);
        }
    }

    fn clamp_scrolling(&mut self, height: usize) {
        let min = 0;
        let max = self.rows.len().saturating_sub(height);
        self.offset = self.offset.clamp(min, max);
    }
}

impl<Id: Eq> InnerListState<Id> {
    fn focusing(&self, id: &Id) -> bool {
        if let Some(cursor) = &self.cursor {
            cursor.id == *id
        } else {
            false
        }
    }
}

impl<Id: Clone + Eq> InnerListState<Id> {
    fn selectable_of_id(&self, id: &Id) -> Option<Cursor<Id>> {
        self.rows.iter().enumerate().find_map(|(i, r)| match r {
            Some(rid) if rid == id => Some(Cursor::new(id.clone(), i)),
            _ => None,
        })
    }

    fn fix_cursor(&mut self) {
        self.cursor = if let Some(cursor) = &self.cursor {
            self.selectable_of_id(&cursor.id)
                .or_else(|| self.selectable_at_or_before_index(cursor.idx))
                .or_else(|| self.selectable_at_or_after_index(cursor.idx))
        } else {
            self.first_selectable()
        }
    }

    /// Bring the list into a state consistent with the current rows and height.
    fn stabilize(&mut self, rows: &[Row<Id>], height: usize) {
        self.rows = rows.iter().map(|r| r.id().cloned()).collect();

        self.fix_cursor();
        if self.make_cursor_visible {
            self.make_cursor_visible(height);
            self.make_cursor_visible = false;
        }

        self.clamp_scrolling(height);
    }
}

pub struct ListState<Id>(Arc<Mutex<InnerListState<Id>>>);

impl<Id> ListState<Id> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InnerListState::new())))
    }

    pub fn list(&self) -> List<Id> {
        List::new(self.0.clone())
    }

    pub fn scroll_up(&mut self) {
        let mut guard = self.0.lock();
        guard.offset = guard.offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        let mut guard = self.0.lock();
        guard.offset = guard.offset.saturating_add(1);
    }
}

impl<Id: Clone> ListState<Id> {
    pub fn cursor(&self) -> Option<Id> {
        self.0.lock().cursor.as_ref().map(|c| c.id.clone())
    }

    pub fn move_cursor_up(&mut self) {
        let mut guard = self.0.lock();
        if let Some(cursor) = &guard.cursor {
            if let Some(new_cursor) = guard.selectable_before_index(cursor.idx) {
                guard.cursor = Some(new_cursor);
                guard.make_cursor_visible = true;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        let mut guard = self.0.lock();
        if let Some(cursor) = &guard.cursor {
            if let Some(new_cursor) = guard.selectable_after_index(cursor.idx) {
                guard.cursor = Some(new_cursor);
                guard.make_cursor_visible = true;
            }
        }
    }
}

////////////
// Widget //
////////////

// TODO Use widgets for rows
#[derive(Debug)]
enum Row<Id> {
    Unselectable(Styled),
    Selectable {
        id: Id,
        normal: Styled,
        normal_bg: ContentStyle,
        selected: Styled,
        selected_bg: ContentStyle,
    },
}

impl<Id> Row<Id> {
    fn id(&self) -> Option<&Id> {
        match self {
            Row::Unselectable(_) => None,
            Row::Selectable { id, .. } => Some(id),
        }
    }

    fn styled(&self) -> &Styled {
        match self {
            Row::Unselectable(styled) => styled,
            Row::Selectable { normal, .. } => normal,
        }
    }
}

pub struct List<Id> {
    state: Arc<Mutex<InnerListState<Id>>>,
    rows: Vec<Row<Id>>,
    focus: bool,
}

impl<Id> List<Id> {
    fn new(state: Arc<Mutex<InnerListState<Id>>>) -> Self {
        Self {
            state,
            rows: vec![],
            focus: false,
        }
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn add_unsel<S: Into<Styled>>(&mut self, styled: S) {
        self.rows.push(Row::Unselectable(styled.into()));
    }

    pub fn add_sel<S1, S2>(
        &mut self,
        id: Id,
        normal: S1,
        normal_bg: ContentStyle,
        selected: S2,
        selected_bg: ContentStyle,
    ) where
        S1: Into<Styled>,
        S2: Into<Styled>,
    {
        self.rows.push(Row::Selectable {
            id,
            normal: normal.into(),
            normal_bg,
            selected: selected.into(),
            selected_bg,
        });
    }
}

#[async_trait]
impl<Id: Clone + Eq + Send> Widget for List<Id> {
    fn size(&self, frame: &mut Frame, _max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let width = self
            .rows
            .iter()
            .map(|r| frame.width_styled(r.styled()))
            .max()
            .unwrap_or(0);
        let height = self.rows.len();
        Size::new(width as u16, height as u16)
    }

    async fn render(self, frame: &mut Frame, pos: Pos, size: Size) {
        let mut guard = self.state.lock();
        guard.stabilize(&self.rows, size.height.into());
        for (i, row) in self.rows.into_iter().enumerate() {
            let dy = i as i32 - guard.offset as i32;
            if dy < 0 || dy >= size.height as i32 {
                break;
            }

            let pos = pos + Pos::new(0, dy);
            match row {
                Row::Unselectable(styled) => frame.write(pos, styled),
                Row::Selectable {
                    id,
                    normal,
                    normal_bg,
                    selected,
                    selected_bg,
                } => {
                    let (fg, bg) = if self.focus && guard.focusing(&id) {
                        (selected, selected_bg)
                    } else {
                        (normal, normal_bg)
                    };
                    frame.write(pos, (" ".repeat(size.width.into()), bg));
                    frame.write(pos, fg);
                }
            }
        }
    }
}
