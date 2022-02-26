use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Modifier, Style};
use tui::widgets::{Block, Borders, Widget};

#[derive(Debug)]
pub struct PaneInfo {
    width: u16,
    hovering: bool,
    dragging: bool,
}

impl Default for PaneInfo {
    fn default() -> Self {
        Self {
            width: 24,
            hovering: false,
            dragging: false,
        }
    }
}

impl PaneInfo {
    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn restrict_width(&mut self, width: u16) {
        self.width = self.width.min(width);
    }

    pub fn hover(&mut self, active: bool) {
        self.hovering = active;
    }

    pub fn drag(&mut self, active: bool) {
        self.dragging = active;
    }

    pub fn drag_to(&mut self, width: u16) {
        if self.dragging {
            self.width = width;
        }
    }
}

// Rendering the pane's border (not part of the pane's area)

struct Border {
    hovering: bool,
}

impl Widget for Border {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut block = Block::default().borders(Borders::LEFT);
        if self.hovering {
            block = block.style(Style::default().add_modifier(Modifier::REVERSED));
        }
        block.render(area, buf);
    }
}

impl PaneInfo {
    pub fn border(&self) -> impl Widget {
        Border {
            hovering: self.hovering,
        }
    }
}
