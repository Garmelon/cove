use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};

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
}

impl Widget for Rooms {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_style = Style::default().add_modifier(Modifier::BOLD);
        let room_style = Style::default().fg(Color::LightBlue);
        let selected_room_style = room_style.add_modifier(Modifier::BOLD);

        let title = if let Some(selected) = self.selected {
            format!("Rooms ({}/{})", selected + 1, self.rooms.len())
        } else {
            format!("Rooms ({})", self.rooms.len())
        };
        let mut lines = vec![Spans::from(Span::styled(title, title_style))];
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
        Paragraph::new(lines).render(area, buf);
    }
}
