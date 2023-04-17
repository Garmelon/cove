use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::{Arc, Mutex};

use crossterm::style::Stylize;
use euphoxide::api::SessionType;
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::conn::{self, Joined};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::widgets::{BoxedAsync, EditorState, Empty, Join2, Text};
use toss::{Style, Styled, Terminal, WidgetExt};

use crate::config::{Config, RoomsSortOrder};
use crate::euph;
use crate::macros::logging_unwrap;
use crate::vault::Vault;

use super::euph::room::EuphRoom;
use super::input::{key, InputEvent, KeyBindingsList};
use super::widgets::{ListBuilder, ListState, Popup};
use super::{util, UiError, UiEvent};

enum State {
    ShowList,
    ShowRoom(String),
    Connect(EditorState),
    Delete(String, EditorState),
}

#[derive(Clone, Copy)]
enum Order {
    Alphabet,
    Importance,
}

impl Order {
    fn from_rooms_sort_order(order: RoomsSortOrder) -> Self {
        match order {
            RoomsSortOrder::Alphabet => Self::Alphabet,
            RoomsSortOrder::Importance => Self::Importance,
        }
    }
}

pub struct Rooms {
    config: &'static Config,

    vault: Vault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    state: State,

    list: ListState<String>,
    order: Order,

    euph_server_config: ServerConfig,
    euph_next_instance_id: usize,
    euph_rooms: HashMap<String, EuphRoom>,
}

impl Rooms {
    pub async fn new(
        config: &'static Config,
        vault: Vault,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        let cookies = logging_unwrap!(vault.euph().cookies().await);
        let euph_server_config = ServerConfig::default().cookies(Arc::new(Mutex::new(cookies)));

        let mut result = Self {
            config,
            vault,
            ui_event_tx,
            state: State::ShowList,
            list: ListState::new(),
            order: Order::from_rooms_sort_order(config.rooms_sort_order),
            euph_server_config,
            euph_next_instance_id: 0,
            euph_rooms: HashMap::new(),
        };

        if !config.offline {
            for (name, config) in &config.euph.rooms {
                if config.autojoin {
                    result.connect_to_room(name.clone());
                }
            }
        }

        result
    }

    fn get_or_insert_room(&mut self, name: String) -> &mut EuphRoom {
        self.euph_rooms.entry(name.clone()).or_insert_with(|| {
            EuphRoom::new(
                self.euph_server_config.clone(),
                self.config.euph_room(&name),
                self.vault.euph().room(name),
                self.ui_event_tx.clone(),
            )
        })
    }

    fn connect_to_room(&mut self, name: String) {
        let room = self.euph_rooms.entry(name.clone()).or_insert_with(|| {
            EuphRoom::new(
                self.euph_server_config.clone(),
                self.config.euph_room(&name),
                self.vault.euph().room(name),
                self.ui_event_tx.clone(),
            )
        });
        room.connect(&mut self.euph_next_instance_id);
    }

    fn connect_to_all_rooms(&mut self) {
        for room in self.euph_rooms.values_mut() {
            room.connect(&mut self.euph_next_instance_id);
        }
    }

    fn disconnect_from_room(&mut self, name: &str) {
        if let Some(room) = self.euph_rooms.get_mut(name) {
            room.disconnect();
        }
    }

    fn disconnect_from_all_rooms(&mut self) {
        for room in self.euph_rooms.values_mut() {
            room.disconnect();
        }
    }

    /// Remove rooms that are not running any more and can't be found in the db
    /// or config. Insert rooms that are in the db or config but not yet in in
    /// the hash map.
    ///
    /// These kinds of rooms are either
    /// - failed connection attempts, or
    /// - rooms that were deleted from the db.
    async fn stabilize_rooms(&mut self) {
        // Collect all rooms from the db and config file
        let rooms = logging_unwrap!(self.vault.euph().rooms().await);
        let mut rooms_set = rooms
            .into_iter()
            .chain(self.config.euph.rooms.keys().cloned())
            .collect::<HashSet<_>>();

        // Prevent room that is currently being shown from being removed. This
        // could otherwise happen after connecting to a room that doesn't exist.
        if let State::ShowRoom(name) = &self.state {
            rooms_set.insert(name.clone());
        }

        // Now `rooms_set` contains all rooms that must exist. Other rooms may
        // also exist, for example rooms that are connecting for the first time.

        self.euph_rooms
            .retain(|n, r| !r.stopped() || rooms_set.contains(n));

        for room in rooms_set {
            self.get_or_insert_room(room).retain();
        }
    }

    pub async fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        match &self.state {
            State::ShowRoom(_) => {}
            _ => self.stabilize_rooms().await,
        }

        match &mut self.state {
            State::ShowList => {
                Self::rooms_widget(&mut self.list, &self.euph_rooms, self.order).await
            }

            State::ShowRoom(name) => {
                self.euph_rooms
                    .get_mut(name)
                    .expect("room exists after stabilization")
                    .widget()
                    .await
            }

            State::Connect(editor) => {
                Self::rooms_widget(&mut self.list, &self.euph_rooms, self.order)
                    .await
                    .below(Self::new_room_widget(editor))
                    .boxed_async()
            }

            State::Delete(name, editor) => {
                Self::rooms_widget(&mut self.list, &self.euph_rooms, self.order)
                    .await
                    .below(Self::delete_room_widget(name, editor))
                    .boxed_async()
            }
        }
    }

    fn new_room_widget(editor: &mut EditorState) -> BoxedAsync<'_, UiError> {
        let room_style = Style::new().bold().blue();

        let inner = Join2::horizontal(
            Text::new(("&", room_style)).segment().with_fixed(true),
            editor
                .widget()
                .with_highlight(|s| Styled::new(s, room_style))
                .segment(),
        );

        Popup::new(inner, "Connect to").boxed_async()
    }

    fn delete_room_widget<'a>(name: &str, editor: &'a mut EditorState) -> BoxedAsync<'a, UiError> {
        let warn_style = Style::new().bold().red();
        let room_style = Style::new().bold().blue();
        let text = Styled::new_plain("Are you sure you want to delete ")
            .then("&", room_style)
            .then(name, room_style)
            .then_plain("?\n\n")
            .then_plain("This will delete the entire room history from your vault. ")
            .then_plain("To shrink your vault afterwards, run ")
            .then("cove gc", Style::new().italic().grey())
            .then_plain(".\n\n")
            .then_plain("To confirm the deletion, ")
            .then_plain("enter the full name of the room and press enter:");

        let inner = Join2::vertical(
            // The Join prevents the text from filling up the entire available
            // space if the editor is wider than the text.
            Join2::horizontal(
                Text::new(text)
                    .resize()
                    .with_max_width(54)
                    .segment()
                    .with_growing(false),
                Empty::new().segment(),
            )
            .segment(),
            Join2::horizontal(
                Text::new(("&", room_style)).segment().with_fixed(true),
                editor
                    .widget()
                    .with_highlight(|s| Styled::new(s, room_style))
                    .segment(),
            )
            .segment(),
        );

        Popup::new(inner, "Delete room")
            .with_border_style(warn_style)
            .boxed_async()
    }

    fn format_pbln(joined: &Joined) -> String {
        let mut p = 0_usize;
        let mut b = 0_usize;
        let mut l = 0_usize;
        let mut n = 0_usize;

        let sessions = joined
            .listing
            .values()
            .map(|s| (s.id(), s.name()))
            .chain(iter::once((
                &joined.session.id,
                &joined.session.name as &str,
            )));
        for (user_id, name) in sessions {
            match user_id.session_type() {
                Some(SessionType::Bot) if name.is_empty() => n += 1,
                Some(SessionType::Bot) => b += 1,
                _ if name.is_empty() => l += 1,
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

    fn format_room_state(state: Option<&euph::State>) -> Option<String> {
        match state {
            None | Some(euph::State::Stopped) => None,
            Some(euph::State::Disconnected) => Some("waiting".to_string()),
            Some(euph::State::Connecting) => Some("connecting".to_string()),
            Some(euph::State::Connected(_, connected)) => match connected {
                conn::State::Joining(joining) if joining.bounce.is_some() => {
                    Some("auth required".to_string())
                }
                conn::State::Joining(_) => Some("joining".to_string()),
                conn::State::Joined(joined) => Some(Self::format_pbln(joined)),
            },
        }
    }

    fn format_unseen_msgs(unseen: usize) -> Option<String> {
        if unseen == 0 {
            None
        } else {
            Some(format!("{unseen}"))
        }
    }

    fn format_room_info(state: Option<&euph::State>, unseen: usize) -> Styled {
        let unseen_style = Style::new().bold().green();

        let state = Self::format_room_state(state);
        let unseen = Self::format_unseen_msgs(unseen);

        match (state, unseen) {
            (None, None) => Styled::default(),
            (None, Some(u)) => Styled::new_plain(" (")
                .then(u, unseen_style)
                .then_plain(")"),
            (Some(s), None) => Styled::new_plain(" (").then_plain(s).then_plain(")"),
            (Some(s), Some(u)) => Styled::new_plain(" (")
                .then_plain(s)
                .then_plain(", ")
                .then(u, unseen_style)
                .then_plain(")"),
        }
    }

    fn sort_rooms(rooms: &mut [(&String, Option<&euph::State>, usize)], order: Order) {
        match order {
            Order::Alphabet => rooms.sort_unstable_by_key(|(name, _, _)| *name),
            Order::Importance => rooms.sort_unstable_by_key(|(name, state, unseen)| {
                (state.is_none(), *unseen == 0, *name)
            }),
        }
    }

    async fn render_rows(
        list_builder: &mut ListBuilder<'_, String, Text>,
        euph_rooms: &HashMap<String, EuphRoom>,
        order: Order,
    ) {
        if euph_rooms.is_empty() {
            list_builder.add_unsel(Text::new((
                "Press F1 for key bindings",
                Style::new().grey().italic(),
            )))
        }

        let mut rooms = vec![];
        for (name, room) in euph_rooms {
            let state = room.room_state();
            let unseen = room.unseen_msgs_count().await;
            rooms.push((name, state, unseen));
        }
        Self::sort_rooms(&mut rooms, order);
        for (name, state, unseen) in rooms {
            let name = name.clone();
            let info = Self::format_room_info(state, unseen);
            list_builder.add_sel(name.clone(), move |selected| {
                let style = if selected {
                    Style::new().bold().black().on_white()
                } else {
                    Style::new().bold().blue()
                };

                let text = Styled::new(format!("&{name}"), style).and_then(info);

                Text::new(text)
            });
        }
    }

    async fn rooms_widget<'a>(
        list: &'a mut ListState<String>,
        euph_rooms: &HashMap<String, EuphRoom>,
        order: Order,
    ) -> BoxedAsync<'a, UiError> {
        let heading_style = Style::new().bold();
        let heading_text =
            Styled::new("Rooms", heading_style).then_plain(format!(" ({})", euph_rooms.len()));

        let mut list_builder = ListBuilder::new();
        Self::render_rows(&mut list_builder, euph_rooms, order).await;

        Join2::vertical(
            Text::new(heading_text).segment().with_fixed(true),
            list_builder.build(list).segment(),
        )
        .boxed_async()
    }

    fn room_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    fn list_showlist_key_bindings(bindings: &mut KeyBindingsList) {
        bindings.heading("Rooms");
        util::list_list_key_bindings(bindings);
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

    fn handle_showlist_input_event(&mut self, event: &InputEvent) -> bool {
        if util::handle_list_input_event(&mut self.list, event) {
            return true;
        }

        match event {
            key!(Enter) => {
                if let Some(name) = self.list.selected() {
                    self.state = State::ShowRoom(name.clone());
                }
                return true;
            }
            key!('c') => {
                if let Some(name) = self.list.selected() {
                    self.connect_to_room(name.clone());
                }
                return true;
            }
            key!('C') => {
                self.connect_to_all_rooms();
                return true;
            }
            key!('d') => {
                if let Some(name) = self.list.selected() {
                    self.disconnect_from_room(&name.clone());
                }
                return true;
            }
            key!('D') => {
                self.disconnect_from_all_rooms();
                return true;
            }
            key!('a') => {
                for (name, options) in &self.config.euph.rooms {
                    if options.autojoin {
                        self.connect_to_room(name.clone());
                    }
                }
                return true;
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
                return true;
            }
            key!('n') => {
                self.state = State::Connect(EditorState::new());
                return true;
            }
            key!('X') => {
                if let Some(name) = self.list.selected() {
                    self.state = State::Delete(name.clone(), EditorState::new());
                }
                return true;
            }
            key!('s') => {
                self.order = match self.order {
                    Order::Alphabet => Order::Importance,
                    Order::Importance => Order::Alphabet,
                };
                return true;
            }
            _ => {}
        }

        false
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        match &self.state {
            State::ShowList => Self::list_showlist_key_bindings(bindings),
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
                util::list_editor_key_bindings(bindings, Self::room_char);
            }
            State::Delete(_, _) => {
                bindings.heading("Rooms");
                bindings.binding("esc", "abort");
                bindings.binding("enter", "delete room");
                util::list_editor_key_bindings(bindings, Self::room_char);
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

        match &mut self.state {
            State::ShowList => {
                if self.handle_showlist_input_event(event) {
                    return true;
                }
            }
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
            }
            State::Connect(ed) => match event {
                key!(Esc) => {
                    self.state = State::ShowList;
                    return true;
                }
                key!(Enter) => {
                    let name = ed.text().to_string();
                    if !name.is_empty() {
                        self.connect_to_room(name.clone());
                        self.state = State::ShowRoom(name);
                    }
                    return true;
                }
                _ => {
                    if util::handle_editor_input_event(ed, terminal, event, Self::room_char) {
                        return true;
                    }
                }
            },
            State::Delete(name, editor) => match event {
                key!(Esc) => {
                    self.state = State::ShowList;
                    return true;
                }
                key!(Enter) if editor.text() == *name => {
                    self.euph_rooms.remove(name);
                    logging_unwrap!(self.vault.euph().room(name.clone()).delete().await);
                    self.state = State::ShowList;
                    return true;
                }
                _ => {
                    if util::handle_editor_input_event(editor, terminal, event, Self::room_char) {
                        return true;
                    }
                }
            },
        }

        false
    }

    pub async fn handle_euph_event(&mut self, event: Event) -> bool {
        let room_name = event.config().room.clone();
        let Some(room) = self.euph_rooms.get_mut(&room_name) else { return false; };

        let handled = room.handle_event(event).await;

        let room_visible = match &self.state {
            State::ShowRoom(name) => *name == room_name,
            _ => true,
        };
        handled && room_visible
    }
}
