use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::{Arc, Mutex};

use cove_config::{Config, Keys, RoomsSortOrder};
use cove_input::InputEvent;
use crossterm::style::Stylize;
use euphoxide::api::SessionType;
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::conn::{self, Joined};
use tokio::sync::mpsc;
use toss::widgets::{BoxedAsync, EditorState, Empty, Join2, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::euph;
use crate::macros::logging_unwrap;
use crate::vault::{EuphVault, RoomIdentifier, Vault};

use super::euph::room::EuphRoom;
use super::widgets::{ListBuilder, ListState, Popup};
use super::{key_bindings, util, UiError, UiEvent};

enum State {
    ShowList,
    ShowRoom(RoomIdentifier),
    Connect(EditorState),
    Delete(RoomIdentifier, EditorState),
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

struct EuphServer {
    config: ServerConfig,
    next_instance_id: usize,
}

impl EuphServer {
    async fn new(vault: &EuphVault, domain: String) -> Self {
        let cookies = logging_unwrap!(vault.cookies(domain.clone()).await);
        let config = ServerConfig::default()
            .domain(domain)
            .cookies(Arc::new(Mutex::new(cookies)));
        Self {
            config,
            next_instance_id: 0,
        }
    }
}

pub struct Rooms {
    config: &'static Config,

    vault: Vault,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    state: State,

    list: ListState<RoomIdentifier>,
    order: Order,

    euph_servers: HashMap<String, EuphServer>,
    euph_rooms: HashMap<RoomIdentifier, EuphRoom>,
}

impl Rooms {
    pub async fn new(
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
            order: Order::from_rooms_sort_order(config.rooms_sort_order),
            euph_servers: HashMap::new(),
            euph_rooms: HashMap::new(),
        };

        if !config.offline {
            for (name, config) in &config.euph.rooms {
                if config.autojoin {
                    result
                        .connect_to_room(RoomIdentifier {
                            // TODO Remove hardcoded domain
                            domain: "euphoria.leet.nu".to_string(),
                            name: name.clone(),
                        })
                        .await;
                }
            }
        }

        result
    }

    async fn get_or_insert_server<'a>(
        vault: &Vault,
        euph_servers: &'a mut HashMap<String, EuphServer>,
        domain: String,
    ) -> &'a mut EuphServer {
        match euph_servers.entry(domain.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let server = EuphServer::new(&vault.euph(), domain).await;
                entry.insert(server)
            }
        }
    }

    async fn get_or_insert_room(&mut self, room: RoomIdentifier) -> &mut EuphRoom {
        let server =
            Self::get_or_insert_server(&self.vault, &mut self.euph_servers, room.domain.clone())
                .await;

        self.euph_rooms.entry(room.clone()).or_insert_with(|| {
            EuphRoom::new(
                self.config,
                server.config.clone(),
                self.config.euph_room(&room.name),
                self.vault.euph().room(room),
                self.ui_event_tx.clone(),
            )
        })
    }

    async fn connect_to_room(&mut self, room: RoomIdentifier) {
        let server =
            Self::get_or_insert_server(&self.vault, &mut self.euph_servers, room.domain.clone())
                .await;

        let room = self.euph_rooms.entry(room.clone()).or_insert_with(|| {
            EuphRoom::new(
                self.config,
                server.config.clone(),
                self.config.euph_room(&room.name),
                self.vault.euph().room(room),
                self.ui_event_tx.clone(),
            )
        });

        room.connect(&mut server.next_instance_id);
    }

    async fn connect_to_all_rooms(&mut self) {
        for (id, room) in &mut self.euph_rooms {
            let server =
                Self::get_or_insert_server(&self.vault, &mut self.euph_servers, id.domain.clone())
                    .await;

            room.connect(&mut server.next_instance_id);
        }
    }

    fn disconnect_from_room(&mut self, room: &RoomIdentifier) {
        if let Some(room) = self.euph_rooms.get_mut(room) {
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
            .chain(self.config.euph.rooms.keys().map(|name| RoomIdentifier {
                // TODO Remove hardcoded domain
                domain: "euphoria.leet.nu".to_string(),
                name: name.clone(),
            }))
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
            self.get_or_insert_room(room).await.retain();
        }
    }

    pub async fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        match &self.state {
            State::ShowRoom(_) => {}
            _ => self.stabilize_rooms().await,
        }

        match &mut self.state {
            State::ShowList => {
                Self::rooms_widget(self.config, &mut self.list, self.order, &self.euph_rooms)
                    .await
                    .desync()
                    .boxed_async()
            }

            State::ShowRoom(id) => {
                self.euph_rooms
                    .get_mut(id)
                    .expect("room exists after stabilization")
                    .widget()
                    .await
            }

            State::Connect(editor) => {
                Self::rooms_widget(self.config, &mut self.list, self.order, &self.euph_rooms)
                    .await
                    .below(Self::new_room_widget(editor))
                    .desync()
                    .boxed_async()
            }

            State::Delete(id, editor) => {
                Self::rooms_widget(self.config, &mut self.list, self.order, &self.euph_rooms)
                    .await
                    // TODO Respect domain
                    .below(Self::delete_room_widget(&id.name, editor))
                    .desync()
                    .boxed_async()
            }
        }
    }

    fn new_room_widget(editor: &mut EditorState) -> impl Widget<UiError> + '_ {
        let room_style = Style::new().bold().blue();

        let inner = Join2::horizontal(
            Text::new(("&", room_style)).segment().with_fixed(true),
            editor
                .widget()
                .with_highlight(|s| Styled::new(s, room_style))
                .segment(),
        );

        Popup::new(inner, "Connect to")
    }

    fn delete_room_widget<'a>(
        name: &str,
        editor: &'a mut EditorState,
    ) -> impl Widget<UiError> + 'a {
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

        Popup::new(inner, "Delete room").with_border_style(warn_style)
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

    fn sort_rooms(rooms: &mut [(&RoomIdentifier, Option<&euph::State>, usize)], order: Order) {
        match order {
            Order::Alphabet => rooms.sort_unstable_by_key(|(id, _, _)| (&id.name, &id.domain)),
            Order::Importance => rooms.sort_unstable_by_key(|(id, state, unseen)| {
                (state.is_none(), *unseen == 0, &id.name, &id.domain)
            }),
        }
    }

    async fn render_rows(
        config: &Config,
        list_builder: &mut ListBuilder<'_, RoomIdentifier, Text>,
        order: Order,
        euph_rooms: &HashMap<RoomIdentifier, EuphRoom>,
    ) {
        if euph_rooms.is_empty() {
            let style = Style::new().grey().italic();
            list_builder.add_unsel(Text::new(
                Styled::new("Press ", style)
                    .and_then(key_bindings::format_binding(&config.keys.general.help))
                    .then(" for key bindings", style),
            ));
        }

        let mut rooms = vec![];
        for (id, room) in euph_rooms {
            let state = room.room_state();
            let unseen = room.unseen_msgs_count().await;
            rooms.push((id, state, unseen));
        }
        Self::sort_rooms(&mut rooms, order);
        for (id, state, unseen) in rooms {
            let id = id.clone();
            let info = Self::format_room_info(state, unseen);
            list_builder.add_sel(id.clone(), move |selected| {
                let style = if selected {
                    Style::new().bold().black().on_white()
                } else {
                    Style::new().bold().blue()
                };

                let text = Styled::new(format!("&{}", id.name), style)
                    .and_then(info)
                    .then(format!(" - {}", id.domain), Style::new().grey());

                Text::new(text)
            });
        }
    }

    async fn rooms_widget<'a>(
        config: &Config,
        list: &'a mut ListState<RoomIdentifier>,
        order: Order,
        euph_rooms: &HashMap<RoomIdentifier, EuphRoom>,
    ) -> impl Widget<UiError> + 'a {
        let heading_style = Style::new().bold();
        let heading_text =
            Styled::new("Rooms", heading_style).then_plain(format!(" ({})", euph_rooms.len()));

        let mut list_builder = ListBuilder::new();
        Self::render_rows(config, &mut list_builder, order, euph_rooms).await;

        Join2::vertical(
            Text::new(heading_text).segment().with_fixed(true),
            list_builder.build(list).segment(),
        )
    }

    fn room_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    async fn handle_showlist_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
    ) -> bool {
        // Open room
        if event.matches(&keys.general.confirm) {
            if let Some(name) = self.list.selected() {
                self.state = State::ShowRoom(name.clone());
            }
            return true;
        }

        // Move cursor and scroll
        if util::handle_list_input_event(&mut self.list, event, keys) {
            return true;
        }

        // Room actions
        if event.matches(&keys.rooms.action.connect) {
            if let Some(name) = self.list.selected() {
                self.connect_to_room(name.clone()).await;
            }
            return true;
        }
        if event.matches(&keys.rooms.action.connect_all) {
            self.connect_to_all_rooms().await;
            return true;
        }
        if event.matches(&keys.rooms.action.disconnect) {
            if let Some(room) = self.list.selected() {
                self.disconnect_from_room(&room.clone());
            }
            return true;
        }
        if event.matches(&keys.rooms.action.disconnect_all) {
            self.disconnect_from_all_rooms();
            return true;
        }
        if event.matches(&keys.rooms.action.connect_autojoin) {
            for (name, options) in &self.config.euph.rooms {
                if options.autojoin {
                    let room = RoomIdentifier {
                        // TODO Remove hardcoded domain
                        domain: "euphoria.leet.nu".to_string(),
                        name: name.clone(),
                    };
                    self.connect_to_room(room).await;
                }
            }
            return true;
        }
        if event.matches(&keys.rooms.action.disconnect_non_autojoin) {
            for (id, room) in &mut self.euph_rooms {
                let autojoin = self
                    .config
                    .euph
                    .rooms
                    .get(&id.name) // TODO Respect domain
                    .map(|r| r.autojoin)
                    .unwrap_or(false);
                if !autojoin {
                    room.disconnect();
                }
            }
            return true;
        }
        if event.matches(&keys.rooms.action.new) {
            self.state = State::Connect(EditorState::new());
            return true;
        }
        if event.matches(&keys.rooms.action.delete) {
            if let Some(room) = self.list.selected() {
                self.state = State::Delete(room.clone(), EditorState::new());
            }
            return true;
        }
        if event.matches(&keys.rooms.action.change_sort_order) {
            self.order = match self.order {
                Order::Alphabet => Order::Importance,
                Order::Importance => Order::Alphabet,
            };
            return true;
        }

        false
    }

    pub async fn handle_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> bool {
        self.stabilize_rooms().await;

        match &mut self.state {
            State::ShowList => {
                if self.handle_showlist_input_event(event, keys).await {
                    return true;
                }
            }
            State::ShowRoom(name) => {
                if let Some(room) = self.euph_rooms.get_mut(name) {
                    if room.handle_input_event(event, keys).await {
                        return true;
                    }
                    if event.matches(&keys.general.abort) {
                        self.state = State::ShowList;
                        return true;
                    }
                }
            }
            State::Connect(editor) => {
                if event.matches(&keys.general.abort) {
                    self.state = State::ShowList;
                    return true;
                }
                if event.matches(&keys.general.confirm) {
                    let name = editor.text().to_string();
                    if !name.is_empty() {
                        let room = RoomIdentifier {
                            // TODO Remove hardcoded domain
                            domain: "euphoria.leet.nu".to_string(),
                            name,
                        };
                        self.connect_to_room(room.clone()).await;
                        self.state = State::ShowRoom(room);
                    }
                    return true;
                }
                if util::handle_editor_input_event(editor, event, keys, Self::room_char) {
                    return true;
                }
            }
            State::Delete(name, editor) => {
                if event.matches(&keys.general.abort) {
                    self.state = State::ShowList;
                    return true;
                }
                if event.matches(&keys.general.confirm) {
                    self.euph_rooms.remove(name);
                    logging_unwrap!(self.vault.euph().room(name.clone()).delete().await);
                    self.state = State::ShowList;
                    return true;
                }
                if util::handle_editor_input_event(editor, event, keys, Self::room_char) {
                    return true;
                }
            }
        }

        false
    }

    pub async fn handle_euph_event(&mut self, event: Event) -> bool {
        let config = event.config();
        let room_id = RoomIdentifier::new(config.server.domain.clone(), config.room.clone());
        let Some(room) = self.euph_rooms.get_mut(&room_id) else {
            return false;
        };

        let handled = room.handle_event(event).await;

        let room_visible = match &self.state {
            State::ShowRoom(id) => *id == room_id,
            _ => true,
        };
        handled && room_visible
    }
}
