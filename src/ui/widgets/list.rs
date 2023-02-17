use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use toss::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

///////////
// State //
///////////

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
struct InnerListState<Id> {
    rows: Vec<Option<Id>>,

    /// Offset of the first line visible on the screen.
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
            make_cursor_visible: true,
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

    fn scroll_so_cursor_is_visible(&mut self, height: usize) {
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

    fn move_cursor_to_make_it_visible(&mut self, height: usize) {
        if let Some(cursor) = &self.cursor {
            let min_idx = self.offset;
            let max_idx = self.offset.saturating_add(height).saturating_sub(1);

            let new_cursor = if cursor.idx < min_idx {
                self.selectable_at_or_after_index(min_idx)
            } else if cursor.idx > max_idx {
                self.selectable_at_or_before_index(max_idx)
            } else {
                return;
            };

            if let Some(new_cursor) = new_cursor {
                self.cursor = Some(new_cursor);
            }
        }
    }

    fn clamp_scrolling(&mut self, height: usize) {
        let min = 0;
        let max = self.rows.len().saturating_sub(height);
        self.offset = self.offset.clamp(min, max);
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
            self.scroll_so_cursor_is_visible(height);
            self.clamp_scrolling(height);
        } else {
            self.clamp_scrolling(height);
            self.move_cursor_to_make_it_visible(height);
        }
        self.make_cursor_visible = true;
    }
}

pub struct ListState<Id>(Arc<Mutex<InnerListState<Id>>>);

impl<Id> ListState<Id> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InnerListState::new())))
    }

    pub fn widget(&self) -> List<Id> {
        List::new(self.0.clone())
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let mut guard = self.0.lock();
        guard.offset = guard.offset.saturating_sub(amount);
        guard.make_cursor_visible = false;
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let mut guard = self.0.lock();
        guard.offset = guard.offset.saturating_add(amount);
        guard.make_cursor_visible = false;
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
            }
        }
        guard.make_cursor_visible = true;
    }

    pub fn move_cursor_down(&mut self) {
        let mut guard = self.0.lock();
        if let Some(cursor) = &guard.cursor {
            if let Some(new_cursor) = guard.selectable_after_index(cursor.idx) {
                guard.cursor = Some(new_cursor);
            }
        }
        guard.make_cursor_visible = true;
    }

    pub fn move_cursor_to_top(&mut self) {
        let mut guard = self.0.lock();
        if let Some(new_cursor) = guard.first_selectable() {
            guard.cursor = Some(new_cursor);
        }
        guard.make_cursor_visible = true;
    }

    pub fn move_cursor_to_bottom(&mut self) {
        let mut guard = self.0.lock();
        if let Some(new_cursor) = guard.last_selectable() {
            guard.cursor = Some(new_cursor);
        }
        guard.make_cursor_visible = true;
    }
}

////////////
// Widget //
////////////

enum Row<Id> {
    Unselectable {
        normal: BoxedWidget,
    },
    Selectable {
        id: Id,
        normal: BoxedWidget,
        selected: BoxedWidget,
    },
}

impl<Id> Row<Id> {
    fn id(&self) -> Option<&Id> {
        match self {
            Self::Unselectable { .. } => None,
            Self::Selectable { id, .. } => Some(id),
        }
    }

    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        match self {
            Self::Unselectable { normal } => normal.size(frame, max_width, max_height),
            Self::Selectable {
                normal, selected, ..
            } => {
                let normal_size = normal.size(frame, max_width, max_height);
                let selected_size = selected.size(frame, max_width, max_height);
                Size::new(
                    normal_size.width.max(selected_size.width),
                    normal_size.height.max(selected_size.height),
                )
            }
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

    pub fn add_unsel<W: Into<BoxedWidget>>(&mut self, normal: W) {
        self.rows.push(Row::Unselectable {
            normal: normal.into(),
        });
    }

    pub fn add_sel<W1, W2>(&mut self, id: Id, normal: W1, selected: W2)
    where
        W1: Into<BoxedWidget>,
        W2: Into<BoxedWidget>,
    {
        self.rows.push(Row::Selectable {
            id,
            normal: normal.into(),
            selected: selected.into(),
        });
    }
}

#[async_trait]
impl<Id: Clone + Eq + Send> Widget for List<Id> {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let width = self
            .rows
            .iter()
            .map(|r| r.size(frame, max_width, Some(1)).width)
            .max()
            .unwrap_or(0);
        let height = self.rows.len();
        Size::new(width, height as u16)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        // Guard acquisition and dropping must be inside its own block or the
        // compiler complains that "future created by async block is not
        // `Send`", pointing to the function body.
        //
        // I assume this is because I'm using the parking lot mutex whose guard
        // is not Send, and even though I was explicitly dropping it with
        // drop(), rustc couldn't figure this out without some help.
        let (offset, cursor) = {
            let mut guard = self.state.lock();
            guard.stabilize(&self.rows, size.height.into());
            (guard.offset as i32, guard.cursor.clone())
        };

        let row_size = Size::new(size.width, 1);
        for (i, row) in self.rows.into_iter().enumerate() {
            let dy = i as i32 - offset;
            if dy < 0 || dy >= size.height as i32 {
                continue;
            }

            frame.push(Pos::new(0, dy), row_size);
            match row {
                Row::Unselectable { normal } => normal.render(frame).await,
                Row::Selectable {
                    id,
                    normal,
                    selected,
                } => {
                    let focusing = self.focus
                        && if let Some(cursor) = &cursor {
                            cursor.id == id
                        } else {
                            false
                        };
                    let widget = if focusing { selected } else { normal };
                    widget.render(frame).await;
                }
            }
            frame.pop();
        }
    }
}
