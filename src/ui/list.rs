use crossterm::style::ContentStyle;
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;

#[derive(Debug)]
pub enum Row<Id> {
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
    pub fn unsel<S: Into<Styled>>(styled: S) -> Self {
        Self::Unselectable(styled.into())
    }

    pub fn sel<S: Into<Styled>>(
        id: Id,
        normal: S,
        normal_bg: ContentStyle,
        selected: S,
        selected_bg: ContentStyle,
    ) -> Self {
        Self::Selectable {
            id,
            normal: normal.into(),
            normal_bg,
            selected: selected.into(),
            selected_bg,
        }
    }

    fn id(&self) -> Option<&Id> {
        match self {
            Row::Unselectable(_) => None,
            Row::Selectable { id, .. } => Some(id),
        }
    }
}

#[derive(Debug)]
pub struct List<Id> {
    cursor: Option<(Id, usize)>,
    offset: usize,
}

// Implemented manually because the derived `Default` requires `Id: Default`.
impl<Id> Default for List<Id> {
    fn default() -> Self {
        Self {
            cursor: Default::default(),
            offset: Default::default(),
        }
    }
}

impl<Id> List<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref().map(|(i, _)| i)
    }
}

impl<Id: Clone + Eq> List<Id> {
    fn first_selectable(rows: &[Row<Id>]) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn last_selectable(rows: &[Row<Id>]) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .rev()
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn selectable_of_id(rows: &[Row<Id>], id: &Id) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .find_map(|(i, r)| r.id().filter(|i| *i == id).map(|c| (c.clone(), i)))
    }

    fn selectable_at_or_before_index(rows: &[Row<Id>], i: usize) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .take(i + 1)
            .rev()
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn selectable_at_or_after_index(rows: &[Row<Id>], i: usize) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .skip(i)
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn selectable_before_index(rows: &[Row<Id>], i: usize) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .take(i)
            .rev()
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn selectable_after_index(rows: &[Row<Id>], i: usize) -> Option<(Id, usize)> {
        rows.iter()
            .enumerate()
            .skip(i + 1)
            .find_map(|(i, r)| r.id().map(|c| (c.clone(), i)))
    }

    fn fix_cursor(&mut self, rows: &[Row<Id>]) {
        self.cursor = if let Some((cid, cidx)) = &self.cursor {
            Self::selectable_of_id(rows, cid)
                .or_else(|| Self::selectable_at_or_before_index(rows, *cidx))
                .or_else(|| Self::selectable_at_or_after_index(rows, *cidx))
        } else {
            Self::first_selectable(rows)
        }
    }

    fn make_cursor_visible(&mut self, height: usize) {
        if let Some(cursor) = &self.cursor {
            // As long as height > 0, min <= max is true
            assert!(height > 0);
            let min = (cursor.1 + 1).saturating_sub(height);
            let max = cursor.1;
            self.offset = self.offset.clamp(min, max);
        }
    }

    fn clamp_scrolling(&mut self, height: usize, rows: usize) {
        let min = 0;
        let max = rows.saturating_sub(height);
        self.offset = self.offset.clamp(min, max);
    }

    /// Bring the list into a state consistent with the current rows and height.
    fn stabilize(&mut self, height: usize, rows: &[Row<Id>]) {
        self.fix_cursor(rows);
        self.clamp_scrolling(height, rows.len());
    }

    pub fn move_cursor_up(&mut self, height: usize, rows: &[Row<Id>]) {
        self.stabilize(height, rows);

        self.cursor = if let Some((_, cidx)) = &self.cursor {
            Self::selectable_before_index(rows, *cidx).or_else(|| Self::first_selectable(rows))
        } else {
            Self::last_selectable(rows)
        };

        self.make_cursor_visible(height);
        self.clamp_scrolling(height, rows.len());
    }

    pub fn move_cursor_down(&mut self, height: usize, rows: &[Row<Id>]) {
        self.stabilize(height, rows);

        self.cursor = if let Some((_, cidx)) = &self.cursor {
            Self::selectable_after_index(rows, *cidx).or_else(|| Self::last_selectable(rows))
        } else {
            Self::first_selectable(rows)
        };

        self.make_cursor_visible(height);
        self.clamp_scrolling(height, rows.len());
    }

    pub fn scroll_up(&mut self, height: usize, rows: &[Row<Id>]) {
        self.stabilize(height, rows);
        self.offset = self.offset.saturating_sub(1);
        self.clamp_scrolling(height, rows.len());
    }

    pub fn scroll_down(&mut self, height: usize, rows: &[Row<Id>]) {
        self.stabilize(height, rows);
        self.offset = self.offset.saturating_add(1);
        self.clamp_scrolling(height, rows.len());
    }

    pub fn render(&mut self, frame: &mut Frame, pos: Pos, size: Size, rows: Vec<Row<Id>>) {
        self.stabilize(size.height as usize, &rows);

        for (i, row) in rows.into_iter().enumerate() {
            let dy = i as i32 - self.offset as i32;
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
                    let (fg, bg) = if self.cursor() == Some(&id) {
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
