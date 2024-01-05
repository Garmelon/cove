mod connect;
mod delete;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::iter;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cove_config::{Config, Keys, RoomsSortOrder};
use cove_input::InputEvent;
use crossterm::style::Stylize;
use euphoxide::api::SessionType;
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::conn::{self, Joined};
use tokio::sync::mpsc;
use toss::widgets::{BoxedAsync, Empty, Join2, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::euph;
use crate::macros::logging_unwrap;
use crate::vault::{EuphVault, RoomIdentifier, Vault};
use crate::version::{NAME, VERSION};

use self::connect::{ConnectResult, ConnectState};
use self::delete::{DeleteResult, DeleteState};

use super::euph::room::EuphRoom;
use super::widgets::{ListBuilder, ListState};
use super::{key_bindings, util, UiError, UiEvent};

enum State {
    ShowList,
    ShowRoom(RoomIdentifier),
    Connect(ConnectState),
    Delete(DeleteState),
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
            .cookies(Arc::new(Mutex::new(cookies)))
            .timeout(Duration::from_secs(10));

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
            for (domain, server) in &config.euph.servers {
                for (name, room) in &server.rooms {
                    if room.autojoin {
                        let id = RoomIdentifier::new(domain.clone(), name.clone());
                        result.connect_to_room(id).await;
                    }
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
                self.config.euph_room(&room.domain, &room.name),
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
                self.config.euph_room(&room.domain, &room.name),
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
        let rooms_from_db = logging_unwrap!(self.vault.euph().rooms().await);
        let rooms_from_config = self
            .config
            .euph
            .servers
            .iter()
            .flat_map(|(domain, server)| {
                server
                    .rooms
                    .keys()
                    .map(|name| RoomIdentifier::new(domain.clone(), name.clone()))
            });
        let mut rooms_set = rooms_from_db
            .into_iter()
            .chain(rooms_from_config)
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
            State::ShowList => Self::rooms_widget(
                &self.vault,
                self.config,
                &mut self.list,
                self.order,
                &self.euph_rooms,
            )
            .await
            .desync()
            .boxed_async(),

            State::ShowRoom(id) => {
                self.euph_rooms
                    .get_mut(id)
                    .expect("room exists after stabilization")
                    .widget()
                    .await
            }

            State::Connect(connect) => Self::rooms_widget(
                &self.vault,
                self.config,
                &mut self.list,
                self.order,
                &self.euph_rooms,
            )
            .await
            .below(connect.widget())
            .desync()
            .boxed_async(),

            State::Delete(delete) => Self::rooms_widget(
                &self.vault,
                self.config,
                &mut self.list,
                self.order,
                &self.euph_rooms,
            )
            .await
            .below(delete.widget())
            .desync()
            .boxed_async(),
        }
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
            Order::Alphabet => rooms.sort_unstable_by_key(|(id, _, _)| *id),
            Order::Importance => rooms
                .sort_unstable_by_key(|(id, state, unseen)| (state.is_none(), *unseen == 0, *id)),
        }
    }

    async fn render_rows(
        list_builder: &mut ListBuilder<'_, RoomIdentifier, Text>,
        order: Order,
        euph_rooms: &HashMap<RoomIdentifier, EuphRoom>,
    ) {
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
                let domain_style = if selected {
                    Style::new().black().on_white()
                } else {
                    Style::new().grey()
                };

                let room_style = if selected {
                    Style::new().bold().black().on_white()
                } else {
                    Style::new().bold().blue()
                };

                let text = Styled::new(format!("{} ", id.domain), domain_style)
                    .then(format!("&{}", id.name), room_style)
                    .and_then(info);

                Text::new(text)
            });
        }
    }

    async fn rooms_widget<'a>(
        vault: &Vault,
        config: &Config,
        list: &'a mut ListState<RoomIdentifier>,
        order: Order,
        euph_rooms: &HashMap<RoomIdentifier, EuphRoom>,
    ) -> impl Widget<UiError> + 'a {
        let version_info = Styled::new_plain("Welcome to ")
            .then(format!("{NAME} {VERSION}"), Style::new().yellow().bold())
            .then_plain("!");
        let help_info = Styled::new("Press ", Style::new().grey())
            .and_then(key_bindings::format_binding(&config.keys.general.help))
            .then(" for key bindings.", Style::new().grey());
        let info = Join2::vertical(
            Text::new(version_info).float().with_center_h().segment(),
            Text::new(help_info).segment(),
        )
        .padding()
        .with_horizontal(1)
        .border();

        let mut heading = Styled::new("Rooms", Style::new().bold());
        let mut title = "Rooms".to_string();

        let total_rooms = euph_rooms.len();
        let connected_rooms = euph_rooms
            .iter()
            .filter(|r| r.1.room_state().is_some())
            .count();
        let total_unseen = logging_unwrap!(vault.euph().total_unseen_msgs_count().await);
        if total_unseen > 0 {
            heading = heading
                .then_plain(format!(" ({connected_rooms}/{total_rooms}, "))
                .then(format!("{total_unseen}"), Style::new().bold().green())
                .then_plain(")");
            title.push_str(&format!(" ({total_unseen})"));
        } else {
            heading = heading.then_plain(format!(" ({connected_rooms}/{total_rooms})"))
        }

        let mut list_builder = ListBuilder::new();
        Self::render_rows(&mut list_builder, order, euph_rooms).await;

        Join2::horizontal(
            Join2::vertical(
                Text::new(heading).segment().with_fixed(true),
                list_builder.build(list).segment(),
            )
            .segment(),
            Join2::vertical(info.segment().with_growing(false), Empty::new().segment())
                .segment()
                .with_growing(false),
        )
        .title(title)
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
            for (domain, server) in &self.config.euph.servers {
                for name in server.rooms.keys() {
                    let id = RoomIdentifier::new(domain.clone(), name.clone());
                    self.connect_to_room(id).await;
                }
            }
            return true;
        }
        if event.matches(&keys.rooms.action.disconnect_non_autojoin) {
            for (id, room) in &mut self.euph_rooms {
                let autojoin = self.config.euph_room(&id.domain, &id.name).autojoin;
                if !autojoin {
                    room.disconnect();
                }
            }
            return true;
        }
        if event.matches(&keys.rooms.action.new) {
            self.state = State::Connect(ConnectState::new());
            return true;
        }
        if event.matches(&keys.rooms.action.delete) {
            if let Some(room) = self.list.selected() {
                self.state = State::Delete(DeleteState::new(room.clone()));
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
            State::Connect(connect) => match connect.handle_input_event(event, keys) {
                ConnectResult::Close => {
                    self.state = State::ShowList;
                    return true;
                }
                ConnectResult::Connect(room) => {
                    self.connect_to_room(room.clone()).await;
                    self.state = State::ShowRoom(room);
                    return true;
                }
                ConnectResult::Handled => {
                    return true;
                }
                ConnectResult::Unhandled => {}
            },
            State::Delete(delete) => match delete.handle_input_event(event, keys) {
                DeleteResult::Close => {
                    self.state = State::ShowList;
                    return true;
                }
                DeleteResult::Delete(room) => {
                    self.euph_rooms.remove(&room);
                    logging_unwrap!(self.vault.euph().room(room).delete().await);
                    self.state = State::ShowList;
                    return true;
                }
                DeleteResult::Handled => {
                    return true;
                }
                DeleteResult::Unhandled => {}
            },
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
