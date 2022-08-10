use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::api::SessionType;
use crate::euph::{Joined, Status};
use crate::vault::Vault;

use super::euph::room::EuphRoom;
use super::input::{key, KeyBindingsList, KeyEvent};
use super::widgets::background::Background;
use super::widgets::border::Border;
use super::widgets::editor::EditorState;
use super::widgets::float::Float;
use super::widgets::join::{HJoin, Segment, VJoin};
use super::widgets::layer::Layer;
use super::widgets::list::{List, ListState};
use super::widgets::padding::Padding;
use super::widgets::text::Text;
use super::widgets::BoxedWidget;
use super::{util, UiEvent};

enum State {
    ShowList,
    ShowRoom(String),
    Connect(EditorState),
}

pub struct Rooms {
    vault: Vault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    state: State,

    list: ListState<String>,
    euph_rooms: HashMap<String, EuphRoom>,
}

impl Rooms {
    pub fn new(vault: Vault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            vault,
            ui_event_tx,
            state: State::ShowList,
            list: ListState::new(),
            euph_rooms: HashMap::new(),
        }
    }

    fn get_or_insert_room(&mut self, name: String) -> &mut EuphRoom {
        self.euph_rooms
            .entry(name.clone())
            .or_insert_with(|| EuphRoom::new(self.vault.euph(name), self.ui_event_tx.clone()))
    }

    /// Remove rooms that are not running any more and can't be found in the db.
    /// Insert rooms that are in the db but not yet in in the hash map.
    ///
    /// These kinds of rooms are either
    /// - failed connection attempts, or
    /// - rooms that were deleted from the db.
    async fn stabilize_rooms(&mut self) {
        let rooms_set = self
            .vault
            .euph_rooms()
            .await
            .into_iter()
            .collect::<HashSet<_>>();
        self.euph_rooms
            .retain(|n, r| !r.stopped() || rooms_set.contains(n));

        for room in rooms_set {
            self.get_or_insert_room(room).retain();
        }
    }

    pub async fn widget(&mut self) -> BoxedWidget {
        match &self.state {
            State::ShowRoom(_) => {}
            _ => self.stabilize_rooms().await,
        }

        match &self.state {
            State::ShowList => self.rooms_widget().await,
            State::ShowRoom(name) => {
                self.euph_rooms
                    .get_mut(name)
                    .expect("room exists after stabilization")
                    .widget()
                    .await
            }
            State::Connect(ed) => {
                let room_style = ContentStyle::default().bold().blue();
                Layer::new(vec![
                    self.rooms_widget().await,
                    Float::new(Border::new(Background::new(VJoin::new(vec![
                        Segment::new(Padding::new(Text::new("Connect to")).horizontal(1)),
                        Segment::new(
                            Padding::new(HJoin::new(vec![
                                Segment::new(Text::new(("&", room_style))),
                                Segment::new(ed.widget().highlight(|s| Styled::new(s, room_style))),
                            ]))
                            .left(1),
                        ),
                    ]))))
                    .horizontal(0.5)
                    .vertical(0.5)
                    .into(),
                ])
                .into()
            }
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

    async fn format_status(room: &EuphRoom) -> Option<String> {
        match room.status().await {
            None => None,
            Some(None) => Some("connecting".to_string()),
            Some(Some(Status::Joining(j))) if j.bounce.is_some() => {
                Some("auth required".to_string())
            }
            Some(Some(Status::Joining(_))) => Some("joining".to_string()),
            Some(Some(Status::Joined(joined))) => Some(Self::format_pbln(&joined)),
        }
    }

    async fn format_unseen_msgs(room: &EuphRoom) -> Option<String> {
        let unseen = room.unseen_msgs_count().await;
        if unseen == 0 {
            None
        } else {
            Some(format!("{unseen}"))
        }
    }

    async fn format_room_info(room: &EuphRoom) -> Styled {
        let unseen_style = ContentStyle::default().bold().green();

        let status = Self::format_status(room).await;
        let unseen = Self::format_unseen_msgs(room).await;

        match (status, unseen) {
            (None, None) => Styled::default(),
            (None, Some(u)) => Styled::new_plain(" (")
                .then(&u, unseen_style)
                .then_plain(")"),
            (Some(s), None) => Styled::new_plain(" (").then_plain(&s).then_plain(")"),
            (Some(s), Some(u)) => Styled::new_plain(" (")
                .then_plain(&s)
                .then_plain(", ")
                .then(&u, unseen_style)
                .then_plain(")"),
        }
    }

    async fn render_rows(&self, list: &mut List<String>) {
        let heading_style = ContentStyle::default().bold();
        let amount = self.euph_rooms.len();
        let heading = Styled::new("Rooms", heading_style).then_plain(format!(" ({amount})"));
        list.add_unsel(Text::new(heading));

        if self.euph_rooms.is_empty() {
            list.add_unsel(Text::new((
                "Press F1 for key bindings",
                ContentStyle::default().grey().italic(),
            )))
        }

        let mut rooms = self.euph_rooms.iter().collect::<Vec<_>>();
        rooms.sort_by_key(|(n, _)| *n);
        for (name, room) in rooms {
            let room_style = ContentStyle::default().bold().blue();
            let room_sel_style = ContentStyle::default().bold().black().on_white();

            let mut normal = Styled::new(format!("&{name}"), room_style);
            let mut selected = Styled::new(format!("&{name}"), room_sel_style);

            let info = Self::format_room_info(room).await;
            normal = normal.and_then(info.clone());
            selected = selected.and_then(info);

            list.add_sel(name.clone(), Text::new(normal), Text::new(selected));
        }
    }

    async fn rooms_widget(&self) -> BoxedWidget {
        let mut list = self.list.widget().focus(true);
        self.render_rows(&mut list).await;
        list.into()
    }

    fn room_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        match &self.state {
            State::ShowList => {
                bindings.heading("Rooms");
                bindings.binding("j/k, ↓/↑", "move cursor up/down");
                bindings.binding("g, home", "move cursor to top");
                bindings.binding("G, end", "move cursor to bottom");
                bindings.binding("ctrl+y/e", "scroll up/down");
                bindings.empty();
                bindings.binding("enter", "enter selected room");
                bindings.binding("c", "connect to selected room");
                bindings.binding("C", "connect to new room");
                bindings.binding("d", "disconnect from selected room");
                bindings.binding("D", "delete room");
            }
            State::ShowRoom(name) => {
                // Key bindings for leaving the room are a part of the room's
                // list_key_bindings function since they may be shadowed by the
                // nick selector or message editor.
                if let Some(room) = self.euph_rooms.get(name) {
                    room.list_key_bindings(bindings).await;
                } else {
                    // There should always be a room here already but I don't
                    // really want to panic in case it is not. If I show a
                    // message like this, it'll hopefully be reported if
                    // somebody ever encounters it.
                    bindings.binding_ctd("oops, this text should never be visible")
                }
            }
            State::Connect(_) => {
                bindings.heading("Rooms");
                bindings.binding("esc", "abort");
                bindings.binding("enter", "connect to room");
                util::list_editor_key_bindings(bindings, Self::room_char, false);
            }
        }
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> bool {
        self.stabilize_rooms().await;

        match &self.state {
            State::ShowList => match event {
                key!('k') | key!(Up) => self.list.move_cursor_up(),
                key!('j') | key!(Down) => self.list.move_cursor_down(),
                key!('g') | key!(Home) => self.list.move_cursor_to_top(),
                key!('G') | key!(End) => self.list.move_cursor_to_bottom(),
                key!(Ctrl + 'y') => self.list.scroll_up(1),
                key!(Ctrl + 'e') => self.list.scroll_down(1),

                key!(Enter) => {
                    if let Some(name) = self.list.cursor() {
                        self.state = State::ShowRoom(name);
                    }
                }
                key!('c') => {
                    if let Some(name) = self.list.cursor() {
                        if let Some(room) = self.euph_rooms.get_mut(&name) {
                            room.connect();
                        }
                    }
                }
                key!('C') => self.state = State::Connect(EditorState::new()),
                key!('d') => {
                    if let Some(name) = self.list.cursor() {
                        if let Some(room) = self.euph_rooms.get_mut(&name) {
                            room.disconnect();
                        }
                    }
                }
                key!('D') => {
                    // TODO Check whether user wanted this via popup
                    if let Some(name) = self.list.cursor() {
                        self.euph_rooms.remove(&name);
                        self.vault.euph(name.clone()).delete();
                    }
                }
                _ => return false,
            },
            State::ShowRoom(name) => {
                if let Some(room) = self.euph_rooms.get_mut(name) {
                    if room.handle_key_event(terminal, crossterm_lock, event).await {
                        return true;
                    }

                    if let key!(Esc) = event {
                        self.state = State::ShowList;
                        return true;
                    }
                }

                return false;
            }
            State::Connect(ed) => match event {
                key!(Esc) => self.state = State::ShowList,
                key!(Enter) => {
                    let name = ed.text();
                    if !name.is_empty() {
                        self.get_or_insert_room(name.clone()).connect();
                        self.state = State::ShowRoom(name);
                    }
                }
                _ => {
                    return util::handle_editor_key_event(
                        ed,
                        terminal,
                        crossterm_lock,
                        event,
                        Self::room_char,
                        false,
                    )
                }
            },
        }

        true
    }
}
