use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{ContentStyle, Stylize};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::api::SessionType;
use crate::euph::{Joined, Status};
use crate::vault::Vault;

use super::room::EuphRoom;
use super::widgets::background::Background;
use super::widgets::list::{List, ListState};
use super::widgets::text::Text;
use super::widgets::BoxedWidget;
use super::{util, UiEvent};

pub struct Rooms {
    vault: Vault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    list: ListState<String>,

    /// If set, a single room is displayed in full instead of the room list.
    focus: Option<String>,

    euph_rooms: HashMap<String, EuphRoom>,
}

impl Rooms {
    pub fn new(vault: Vault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            vault,
            ui_event_tx,
            list: ListState::new(),
            focus: None,
            euph_rooms: HashMap::new(),
        }
    }

    /// Remove rooms that are not running any more and can't be found in the db.
    ///
    /// These kinds of rooms are either
    /// - failed connection attempts, or
    /// - rooms that were deleted from the db.
    async fn stabilize_rooms(&mut self) -> Vec<String> {
        let mut rooms = self.vault.euph_rooms().await;
        let rooms_set = rooms.iter().map(|n| n as &str).collect::<HashSet<_>>();
        self.euph_rooms
            .retain(|n, r| !r.stopped() || rooms_set.contains(n as &str));

        for room in self.euph_rooms.values_mut() {
            room.retain();
        }

        for room in self.euph_rooms.keys() {
            rooms.push(room.clone());
        }
        rooms.sort_unstable();
        rooms.dedup();
        rooms
    }

    pub async fn widget(&mut self) -> BoxedWidget {
        if let Some(room) = &self.focus {
            let actual_room = self.euph_rooms.entry(room.clone()).or_insert_with(|| {
                EuphRoom::new(self.vault.euph(room.clone()), self.ui_event_tx.clone())
            });
            actual_room.widget().await
        } else {
            self.rooms_widget().await
        }
    }

    fn format_pbln(joined: &Joined) -> String {
        let mut p = 0_usize;
        let mut b = 0_usize;
        let mut l = 0_usize;
        let mut n = 0_usize;
        for sess in iter::once(&joined.session).chain(joined.listing.values()) {
            match sess.id.session_type() {
                Some(SessionType::Bot) if sess.name.is_empty() => n += 1,
                Some(SessionType::Bot) => b += 1,
                _ if sess.name.is_empty() => l += 1,
                _ => p += 1,
            }
        }

        // There must always be either one p, b, l or n since we're including
        // ourselves.
        let mut result = vec![];
        if p > 0 {
            result.push(format!("{p}p"));
        }
        if b > 0 {
            result.push(format!("{b}b"));
        }
        if l > 0 {
            result.push(format!("{l}l"));
        }
        if n > 0 {
            result.push(format!("{n}n"));
        }
        result.join(" ")
    }

    fn format_status(status: &Option<Status>) -> String {
        match status {
            None => " (connecting)".to_string(),
            Some(Status::Joining(j)) if j.bounce.is_some() => " (auth required)".to_string(),
            Some(Status::Joining(_)) => " (joining)".to_string(),
            Some(Status::Joined(j)) => format!(" ({})", Self::format_pbln(j)),
        }
    }

    async fn render_rows(&self, list: &mut List<String>, rooms: Vec<String>) {
        let heading_style = ContentStyle::default().bold();
        let heading = Styled::new(("Rooms", heading_style)).then(format!(" ({})", rooms.len()));
        list.add_unsel(Text::new(heading));

        for room in rooms {
            let bg_style = ContentStyle::default();
            let bg_sel_style = ContentStyle::default().black().on_white();
            let room_style = ContentStyle::default().bold().blue();
            let room_sel_style = ContentStyle::default().bold().black().on_white();

            let mut normal = Styled::new((format!("&{room}"), room_style));
            let mut selected = Styled::new((format!("&{room}"), room_sel_style));
            if let Some(room) = self.euph_rooms.get(&room) {
                if let Some(status) = room.status().await {
                    let status = Self::format_status(&status);
                    normal = normal.then((status.clone(), bg_style));
                    selected = selected.then((status, bg_sel_style));
                }
            };

            list.add_sel(
                room,
                Text::new(normal),
                Background::new(Text::new(selected), bg_sel_style),
            );
        }
    }

    async fn rooms_widget(&mut self) -> BoxedWidget {
        let rooms = self.stabilize_rooms().await;
        let mut list = self.list.list().focus(true);
        self.render_rows(&mut list, rooms).await;
        list.into()
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) {
        if let Some(room) = &self.focus {
            if event.code == KeyCode::Enter {
                self.focus = None;
            } else {
                let actual_room = self.euph_rooms.entry(room.clone()).or_insert_with(|| {
                    EuphRoom::new(self.vault.euph(room.clone()), self.ui_event_tx.clone())
                });
                actual_room
                    .handle_key_event(terminal, crossterm_lock, event)
                    .await;
            }
        } else {
            match event.code {
                KeyCode::Enter => self.focus = self.list.cursor(),
                KeyCode::Char('j') | KeyCode::Down => self.list.move_cursor_down(),
                KeyCode::Char('k') | KeyCode::Up => self.list.move_cursor_up(),
                KeyCode::Char('J') => self.list.scroll_down(), // TODO Replace by Ctrl+E and mouse scroll
                KeyCode::Char('K') => self.list.scroll_up(), // TODO Replace by Ctrl+Y and mouse scroll
                KeyCode::Char('c') => {
                    if let Some(name) = self.list.cursor() {
                        let room = self.euph_rooms.entry(name.clone()).or_insert_with(|| {
                            EuphRoom::new(self.vault.euph(name.clone()), self.ui_event_tx.clone())
                        });
                        room.connect();
                    }
                }
                KeyCode::Char('C') => {
                    if let Some(name) = util::prompt(terminal, crossterm_lock) {
                        let name = name.trim().to_string();
                        let room = self.euph_rooms.entry(name.clone()).or_insert_with(|| {
                            EuphRoom::new(self.vault.euph(name), self.ui_event_tx.clone())
                        });
                        room.connect();
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(name) = self.list.cursor() {
                        let room = self.euph_rooms.entry(name.clone()).or_insert_with(|| {
                            EuphRoom::new(self.vault.euph(name.clone()), self.ui_event_tx.clone())
                        });
                        room.disconnect();
                    }
                }
                KeyCode::Char('D') => {
                    if let Some(name) = self.list.cursor() {
                        self.euph_rooms.remove(&name);
                        self.vault.euph(name.clone()).delete();
                    }
                }
                _ => {}
            }
        }
    }
}
