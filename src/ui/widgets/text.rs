use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;
use toss::widthdb::WidthDb;

use super::Widget;

pub struct Text {
    styled: Styled,
    wrap: bool,
}

impl Text {
    pub fn new<S: Into<Styled>>(styled: S) -> Self {
        Self {
            styled: styled.into(),
            wrap: false,
        }
    }

    pub fn wrap(mut self, active: bool) -> Self {
        // TODO Re-think and check what behaviour this setting should entail
        self.wrap = active;
        self
    }

    fn wrapped(&self, widthdb: &mut WidthDb, max_width: Option<u16>) -> Vec<Styled> {
        let max_width = if self.wrap {
            max_width.map(|w| w as usize).unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };

        let indices = widthdb.wrap(self.styled.text(), max_width);
        self.styled.clone().split_at_indices(&indices)
    }
}

#[async_trait]
impl Widget for Text {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let lines = self.wrapped(frame.widthdb(), max_width);
        let widthdb = frame.widthdb();
        let min_width = lines
            .iter()
            .map(|l| widthdb.width(l.text().trim_end()))
            .max()
            .unwrap_or(0);
        let min_height = lines.len();
        Size::new(min_width as u16, min_height as u16)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        for (i, line) in self
            .wrapped(frame.widthdb(), Some(size.width))
            .into_iter()
            .enumerate()
        {
            frame.write(Pos::new(0, i as i32), line);
        }
    }
}
