use std::cmp;

use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Modifier, Style};
use tui::widgets::{Block, Borders, StatefulWidget, Widget};

pub struct Rooms {}

impl Rooms {
    pub fn new() -> Self {
        Self {}
    }
}

impl StatefulWidget for Rooms {
    type State = RoomsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.width = cmp::min(state.width, area.width);

        // let left = Rect {
        //     width: area.width - 1,
        //     ..area
        // };
        let right = Rect {
            x: area.right() - 1,
            width: 1,
            ..area
        };
        let style = if state.hovering {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        Block::default()
            .borders(Borders::RIGHT)
            .style(style)
            .render(right, buf);
    }
}

#[derive(Debug)]
pub struct RoomsState {
    width: u16,
    offset: u16,
    hovering: bool,
    dragging: bool,
}

impl Default for RoomsState {
    fn default() -> Self {
        Self {
            width: 24,
            offset: 0,
            hovering: false,
            dragging: false,
        }
    }
}

impl RoomsState {
    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn hover(&mut self, active: bool) {
        self.hovering = active;
    }

    pub fn drag(&mut self, active: bool) {
        self.dragging = active;
    }

    pub fn dragging(&self) -> bool {
        self.dragging
    }

    pub fn drag_to(&mut self, width: u16) {
        if self.dragging {
            self.width = width;
        }
    }
}
