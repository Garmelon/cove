use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

use crate::chat::Chat;
use crate::euph;
use crate::vault::{EuphMsg, EuphVault, Vault};

use super::UiEvent;

mod style {
    use crossterm::style::{ContentStyle, Stylize};

    pub fn room() -> ContentStyle {
        ContentStyle::default().bold().blue()
    }

    pub fn room_inverted() -> ContentStyle {
        ContentStyle::default().bold().black().on_white()
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Cursor {
    index: usize,
    line: i32,
}

pub struct Rooms {
    vault: Vault,

    /// Cursor position inside the room list.
    ///
    /// If there are any rooms, this should point to a valid room.
    cursor: Option<Cursor>,

    /// If set, a single room is displayed in full instead of the room list.
    focus: Option<String>,

    euph_rooms: HashMap<String, euph::Room>,
    euph_chats: HashMap<String, Chat<EuphMsg, EuphVault>>,
}

impl Rooms {
    pub fn new(vault: Vault) -> Self {
        Self {
            vault,
            cursor: None,
            focus: None,
            euph_rooms: HashMap::new(),
            euph_chats: HashMap::new(),
        }
    }

    async fn rooms(&self) -> Vec<String> {
        let mut rooms = HashSet::new();
        for room in self.vault.euph_rooms().await {
            rooms.insert(room);
        }
        for room in self.euph_rooms.keys().cloned() {
            rooms.insert(room);
        }
        let mut rooms = rooms.into_iter().collect::<Vec<_>>();
        rooms.sort_unstable();
        rooms
    }

    fn make_cursor_consistent(&mut self, rooms: &[String], height: i32) {
        // Fix index if it's wrong
        if rooms.is_empty() {
            self.cursor = None;
        } else if let Some(cursor) = &mut self.cursor {
            let max_index = rooms.len() - 1;
            if cursor.index > max_index {
                cursor.index = max_index;
            }
        } else {
            self.cursor = Some(Cursor::default());
        }

        // Fix line if it's wrong
        if let Some(cursor) = &mut self.cursor {
            cursor.line = cursor
                .line
                // Make sure the cursor is visible on screen
                .clamp(0, height - 1)
                // Make sure there is no free space below the room list:
                // height - line <= len - index
                // height - len + index <= line
                .max(height - rooms.len() as i32 + cursor.index as i32)
                // Make sure there is no free space above the room list:
                // line <= index
                .min(cursor.index as i32);
        }
    }

    fn make_euph_rooms_consistent(&mut self, rooms: &[String]) {
        let rooms = rooms
            .iter()
            .map(|n| n.to_string())
            .collect::<HashSet<String>>();

        self.euph_rooms
            .retain(|n, r| rooms.contains(n) && !r.stopped());
        self.euph_chats.retain(|n, _| rooms.contains(n));
    }

    fn make_consistent(&mut self, rooms: &[String], height: i32) {
        self.make_cursor_consistent(rooms, height);
        self.make_euph_rooms_consistent(rooms);
    }

    pub async fn render(&mut self, frame: &mut Frame) {
        if let Some(room) = &self.focus {
            let chat = self
                .euph_chats
                .entry(room.clone())
                .or_insert_with(|| Chat::new(self.vault.euph(room.clone())));
            chat.render(frame, Pos::new(0, 0), frame.size()).await;
        } else {
            self.render_rooms(frame).await;
        }
    }

    async fn render_rooms(&mut self, frame: &mut Frame) {
        let size = frame.size();

        let rooms = self.rooms().await;
        self.make_consistent(&rooms, size.height.into());

        let cursor = self.cursor.unwrap_or_default();
        for (index, room) in rooms.iter().enumerate() {
            let y = index as i32 - cursor.index as i32 + cursor.line;

            let style = if index == cursor.index {
                style::room_inverted()
            } else {
                style::room()
            };

            for x in 0..size.width {
                frame.write(Pos::new(x.into(), y), " ", style);
            }
            let suffix = if self.euph_rooms.contains_key(room) {
                "*"
            } else {
                ""
            };
            let room_str = format!("&{room}{suffix}");
            frame.write(Pos::new(0, y), &room_str, style);
        }
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        size: Size,
        ui_event_tx: &mpsc::UnboundedSender<UiEvent>,
        event: KeyEvent,
    ) {
        if let Some(focus) = &self.focus {
            if event.code == KeyCode::Esc {
                self.focus = None;
            }
        } else {
            let rooms = self.rooms().await;
            self.make_consistent(&rooms, size.height.into());

            match event.code {
                KeyCode::Enter => {
                    if let Some(cursor) = self.cursor {
                        if let Some(room) = rooms.get(cursor.index) {
                            self.focus = Some(room.clone());
                        }
                    }
                }
                KeyCode::Char('j') => {
                    if let Some(cursor) = &mut self.cursor {
                        cursor.index = cursor.index.saturating_add(1);
                        cursor.line += 1;
                    }
                }
                KeyCode::Char('k') => {
                    if let Some(cursor) = &mut self.cursor {
                        cursor.index = cursor.index.saturating_sub(1);
                        cursor.line -= 1;
                    }
                }
                KeyCode::Char('c') => {
                    if let Some(cursor) = &self.cursor {
                        if let Some(room) = rooms.get(cursor.index) {
                            let room = room.clone();
                            self.euph_rooms.entry(room.clone()).or_insert_with(|| {
                                euph::Room::new(
                                    room.clone(),
                                    self.vault.euph(room),
                                    ui_event_tx.clone(),
                                )
                            });
                        }
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(cursor) = &self.cursor {
                        if let Some(room) = rooms.get(cursor.index) {
                            self.euph_rooms.remove(room);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
