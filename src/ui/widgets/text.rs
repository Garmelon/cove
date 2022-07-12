use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;

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

    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    fn wrapped(&self, frame: &mut Frame, max_width: Option<u16>) -> Vec<Styled> {
        let max_width = if self.wrap {
            max_width.map(|w| w as usize).unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };

        let indices = frame.wrap(&self.styled.text(), max_width);
        self.styled.clone().split_at_indices(&indices)
    }
}

#[async_trait]
impl Widget for Text {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let lines = self.wrapped(frame, max_width);
        let min_width = lines
            .iter()
            .map(|l| frame.width(&l.text()))
            .max()
            .unwrap_or(0);
        let min_height = lines.len();
        Size::new(min_width as u16, min_height as u16)
    }

    async fn render(&self, frame: &mut Frame, pos: Pos, size: Size) {
        for (i, line) in self
            .wrapped(frame, Some(size.width))
            .into_iter()
            .enumerate()
        {
            frame.write(pos + Pos::new(0, i as i32), line);
        }
    }
}
