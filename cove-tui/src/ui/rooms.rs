use std::cmp;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Paragraph, StatefulWidget, Widget};

use crate::room::Room;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RoomInfo {
    name: String,
}

pub struct Rooms {
    rooms: Vec<RoomInfo>,
    selected: Option<usize>,
}

impl Rooms {
    pub fn new(rooms: &HashMap<String, Arc<Mutex<Room>>>) -> Self {
        let mut rooms = rooms
            .iter()
            .map(|(name, _room)| RoomInfo { name: name.clone() })
            .collect::<Vec<_>>();
        rooms.sort();
        Self {
            rooms,
            selected: None,
        }
    }

    pub fn select(mut self, name: &str) -> Self {
        for (i, room) in self.rooms.iter().enumerate() {
            if room.name == name {
                self.selected = Some(i);
            }
        }
        self
    }

    pub fn dummy() -> Self {
        fn r(s: &str) -> RoomInfo {
            RoomInfo {
                name: s.to_string(),
            }
        }

        let mut rooms = vec![r("xkcd"), r("test"), r("welcome"), r("music")];
        rooms.sort();
        Rooms {
            rooms,
            selected: None,
        }
        .select("welcome")
    }
}

impl StatefulWidget for Rooms {
    type State = RoomsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let title_style = Style::default().add_modifier(Modifier::BOLD);
        let empty_style = Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::ITALIC);
        let room_style = Style::default().fg(Color::LightBlue);
        let selected_room_style = room_style.add_modifier(Modifier::BOLD);

        state.width = cmp::min(state.width, area.width);

        // Actual room names
        let left = Rect {
            width: area.width - 1,
            ..area
        };
        let mut lines = vec![Spans::from(Span::styled("Rooms", title_style))];
        if self.rooms.is_empty() {
            lines.push(Spans::from(vec![
                Span::raw("\r\n"),
                Span::styled("none", empty_style),
            ]));
        }
        for (i, room) in self.rooms.iter().enumerate() {
            let name = format!("&{}", room.name);
            if Some(i) == self.selected {
                lines.push(Spans::from(vec![
                    Span::raw("\n>"),
                    Span::styled(name, selected_room_style),
                ]));
            } else {
                lines.push(Spans::from(vec![
                    Span::raw("\n "),
                    Span::styled(name, room_style),
                ]));
            }
        }
        Paragraph::new(lines).render(left, buf);

        // The panel's border
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

// TODO Figure out some sort of scroll offset solution
#[derive(Debug)]
pub struct RoomsState {
    width: u16,
    hovering: bool,
    dragging: bool,
}

impl Default for RoomsState {
    fn default() -> Self {
        Self {
            width: 24,
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

    pub fn drag_to(&mut self, width: u16) {
        if self.dragging {
            self.width = width;
        }
    }
}
