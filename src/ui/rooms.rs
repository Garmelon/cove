use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::Arc;

use crossterm::style::{ContentStyle, Stylize};
use euphoxide::api::SessionType;
use euphoxide::conn::{Joined, Status};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::config::Config;
use crate::euph::EuphRoomEvent;
use crate::vault::Vault;

use super::euph::room::{EuphRoom, RoomStatus};
use super::input::{key, InputEvent, KeyBindingsList};
use super::widgets::editor::EditorState;
use super::widgets::join::{HJoin, Segment, VJoin};
use super::widgets::layer::Layer;
use super::widgets::list::{List, ListState};
use super::widgets::popup::Popup;
use super::widgets::resize::Resize;
use super::widgets::text::Text;
use super::widgets::BoxedWidget;
use super::{util, UiEvent};

enum State {
    ShowList,
    ShowRoom(String),
    Connect(EditorState),
    Delete(String, EditorState),
}

enum Order {
    Alphabet,
    Importance,
}

pub struct Rooms {
    config: &'static Config,

    vault: Vault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    state: State,

    list: ListState<String>,
    order: Order,
    euph_rooms: HashMap<String, EuphRoom>,
}

impl Rooms {
    pub fn new(
        config: &'static Config,
        vault: Vault,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        let mut result = Self {
            config,
            vault,
            ui_event_tx,
            state: State::ShowList,
            list: ListState::new(),
            order: Order::Alphabet,
            euph_rooms: HashMap::new(),
        };

        if !config.offline {
            for (name, config) in &config.euph.rooms {
                if config.autojoin {
                    result.get_or_insert_room(name.clone()).connect();
                }
            }
        }

        result
    }

    fn get_or_insert_room(&mut self, name: String) -> &mut EuphRoom {
        self.euph_rooms.entry(name.clone()).or_insert_with(|| {
            EuphRoom::new(
                self.config.euph_room(&name),
                self.vault.euph(name),
                self.ui_event_tx.clone(),
            )
        })
    }

    /// Remove rooms that are not running any more and can't be found in the db.
    /// Insert rooms that are in the db but not yet in in the hash map.
    ///
    /// These kinds of rooms are either
    /// - failed connection attempts, or
    /// - rooms that were deleted from the db.
    async fn stabilize_rooms(&mut self) {
        let mut rooms_set = self
            .vault
            .euph_rooms()
            .await
            .into_iter()
            .collect::<HashSet<_>>();

        // Prevent room that is currently being shown from being removed. This
        // could otherwise happen when connecting to a room that doesn't exist.
        if let State::ShowRoom(name) = &self.state {
            rooms_set.insert(name.clone());
        }

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
            State::Connect(editor) => Layer::new(vec![
                self.rooms_widget().await,
                Self::new_room_widget(editor),
            ])
            .into(),
            State::Delete(name, editor) => Layer::new(vec![
                self.rooms_widget().await,
                Self::delete_room_widget(name, editor),
            ])
            .into(),
        }
    }

    fn new_room_widget(editor: &EditorState) -> BoxedWidget {
        let room_style = ContentStyle::default().bold().blue();
        let editor = editor.widget().highlight(|s| Styled::new(s, room_style));
        Popup::new(HJoin::new(vec![
            Segment::new(Text::new(("&", room_style))),
            Segment::new(editor).priority(0),
        ]))
        .title("Connect to")
        .build()
    }

    fn delete_room_widget(name: &str, editor: &EditorState) -> BoxedWidget {
        let warn_style = ContentStyle::default().bold().red();
        let room_style = ContentStyle::default().bold().blue();
        let editor = editor.widget().highlight(|s| Styled::new(s, room_style));
        let text = Styled::new_plain("Are you sure you want to delete ")
            .then("&", room_style)
            .then(name, room_style)
            .then_plain("?\n\n")
            .then_plain("This will delete the entire room history from your vault. ")
            .then_plain("To shrink your vault afterwards, run ")
            .then("cove gc", ContentStyle::default().italic().grey())
            .then_plain(".\n\n")
            .then_plain("To confirm the deletion, ")
            .then_plain("enter the full name of the room and press enter:");
        Popup::new(VJoin::new(vec![
            // The HJoin prevents the text from filling up the entire available
            // space if the editor is wider than the text.
            Segment::new(HJoin::new(vec![Segment::new(
                Resize::new(Text::new(text).wrap(true)).max_width(54),
            )])),
            Segment::new(HJoin::new(vec![
                Segment::new(Text::new(("&", room_style))),
                Segment::new(editor).priority(0),
            ])),
        ]))
        .title(("Delete room", warn_style))
        .border(warn_style)
        .build()
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

    fn format_status(status: RoomStatus) -> Option<String> {
        match status {
            RoomStatus::NoRoom | RoomStatus::Stopped => None,
            RoomStatus::Connecting => Some("connecting".to_string()),
            RoomStatus::Connected(Status::Joining(j)) if j.bounce.is_some() => {
                Some("auth required".to_string())
            }
            RoomStatus::Connected(Status::Joining(_)) => Some("joining".to_string()),
            RoomStatus::Connected(Status::Joined(joined)) => Some(Self::format_pbln(&joined)),
        }
    }

    fn format_unseen_msgs(unseen: usize) -> Option<String> {
        if unseen == 0 {
            None
        } else {
            Some(format!("{unseen}"))
        }
    }

    fn format_room_info(status: RoomStatus, unseen: usize) -> Styled {
        let unseen_style = ContentStyle::default().bold().green();

        let status = Self::format_status(status);
        let unseen = Self::format_unseen_msgs(unseen);

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

    fn sort_rooms(&self, rooms: &mut [(&String, RoomStatus, usize)]) {
        match self.order {
            Order::Alphabet => rooms.sort_unstable_by_key(|(n, _, _)| *n),
            Order::Importance => {
                rooms.sort_unstable_by_key(|(n, s, u)| (!s.connecting_or_connected(), *u == 0, *n))
            }
        }
    }

    async fn render_rows(&self, list: &mut List<String>) {
        if self.euph_rooms.is_empty() {
            list.add_unsel(Text::new((
                "Press F1 for key bindings",
                ContentStyle::default().grey().italic(),
            )))
        }

        let mut rooms = vec![];
        for (name, room) in &self.euph_rooms {
            let status = room.status().await;
            let unseen = room.unseen_msgs_count().await;
            rooms.push((name, status, unseen));
        }
        self.sort_rooms(&mut rooms);
        for (name, status, unseen) in rooms {
            let room_style = ContentStyle::default().bold().blue();
            let room_sel_style = ContentStyle::default().bold().black().on_white();

            let mut normal = Styled::new(format!("&{name}"), room_style);
            let mut selected = Styled::new(format!("&{name}"), room_sel_style);

            let info = Self::format_room_info(status, unseen);
            normal = normal.and_then(info.clone());
            selected = selected.and_then(info);

            list.add_sel(name.clone(), Text::new(normal), Text::new(selected));
        }
    }

    async fn rooms_widget(&self) -> BoxedWidget {
        let heading_style = ContentStyle::default().bold();
        let amount = self.euph_rooms.len();
        let heading =
            Text::new(Styled::new("Rooms", heading_style).then_plain(format!(" ({amount})")));

        let mut list = self.list.widget().focus(true);
        self.render_rows(&mut list).await;

        VJoin::new(vec![Segment::new(heading), Segment::new(list).priority(0)]).into()
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
                bindings.binding("C", "connect to all rooms");
                bindings.binding("d", "disconnect from selected room");
                bindings.binding("D", "disconnect from all rooms");
                bindings.binding("a", "connect to all autojoin room");
                bindings.binding("A", "disconnect from all non-autojoin rooms");
                bindings.binding("n", "connect to new room");
                bindings.binding("X", "delete room");
                bindings.empty();
                bindings.binding("s", "change sort order");
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
            State::Delete(_, _) => {
                bindings.heading("Rooms");
                bindings.binding("esc", "abort");
                bindings.binding("enter", "delete room");
                util::list_editor_key_bindings(bindings, Self::room_char, false);
            }
        }
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
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
                key!('C') => {
                    for room in self.euph_rooms.values_mut() {
                        room.connect();
                    }
                }
                key!('d') => {
                    if let Some(name) = self.list.cursor() {
                        if let Some(room) = self.euph_rooms.get_mut(&name) {
                            room.disconnect();
                        }
                    }
                }
                key!('D') => {
                    for room in self.euph_rooms.values_mut() {
                        room.disconnect();
                    }
                }
                key!('a') => {
                    for (name, options) in &self.config.euph.rooms {
                        if options.autojoin {
                            self.get_or_insert_room(name.clone()).connect();
                        }
                    }
                }
                key!('A') => {
                    for (name, room) in &mut self.euph_rooms {
                        let autojoin = self
                            .config
                            .euph
                            .rooms
                            .get(name)
                            .map(|r| r.autojoin)
                            .unwrap_or(false);
                        if !autojoin {
                            room.disconnect();
                        }
                    }
                }
                key!('n') => self.state = State::Connect(EditorState::new()),
                key!('X') => {
                    if let Some(name) = self.list.cursor() {
                        self.state = State::Delete(name, EditorState::new());
                    }
                }
                key!('s') => {
                    self.order = match self.order {
                        Order::Alphabet => Order::Importance,
                        Order::Importance => Order::Alphabet,
                    };
                }
                _ => return false,
            },
            State::ShowRoom(name) => {
                if let Some(room) = self.euph_rooms.get_mut(name) {
                    if room
                        .handle_input_event(terminal, crossterm_lock, event)
                        .await
                    {
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
                    return util::handle_editor_input_event(
                        ed,
                        terminal,
                        crossterm_lock,
                        event,
                        Self::room_char,
                        false,
                    )
                }
            },
            State::Delete(name, editor) => match event {
                key!(Esc) => self.state = State::ShowList,
                key!(Enter) if editor.text() == *name => {
                    self.euph_rooms.remove(name);
                    self.vault.euph(name.clone()).delete();
                    self.state = State::ShowList;
                }
                _ => {
                    return util::handle_editor_input_event(
                        editor,
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

    pub fn handle_euph_room_event(&mut self, name: String, event: EuphRoomEvent) -> bool {
        let room_visible = if let State::ShowRoom(n) = &self.state {
            *n == name
        } else {
            true
        };

        let room = self.get_or_insert_room(name);
        let handled = room.handle_euph_room_event(event);
        handled && room_visible
    }
}
