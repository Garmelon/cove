use std::collections::VecDeque;

use cove_config::{Config, Keys};
use cove_input::InputEvent;
use crossterm::style::Stylize;
use euphoxide::api::{Data, Message, MessageId, PacketType, SessionId};
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::conn::{self, Joined, Joining, SessionInfo};
use jiff::tz::TimeZone;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use toss::widgets::{BoxedAsync, EditorState, Join2, Layer, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::euph;
use crate::macros::logging_unwrap;
use crate::ui::chat::{ChatState, Reaction};
use crate::ui::widgets::ListState;
use crate::ui::{util, UiError, UiEvent};
use crate::vault::EuphRoomVault;

use super::account::AccountUiState;
use super::links::LinksState;
use super::popup::{PopupResult, RoomPopup};
use super::{auth, inspect, nick, nick_list};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Chat,
    NickList,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Normal,
    Auth(EditorState),
    Nick(EditorState),
    Account(AccountUiState),
    Links(LinksState),
    InspectMessage(Message),
    InspectSession(SessionInfo),
}

type EuphChatState = ChatState<euph::SmallMessage, EuphRoomVault>;

pub struct EuphRoom {
    config: &'static Config,
    server_config: ServerConfig,
    room_config: cove_config::EuphRoom,
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    room: Option<euph::Room>,

    focus: Focus,
    state: State,
    popups: VecDeque<RoomPopup>,

    chat: EuphChatState,
    last_msg_sent: Option<oneshot::Receiver<MessageId>>,

    nick_list: ListState<SessionId>,
}

impl EuphRoom {
    pub fn new(
        config: &'static Config,
        server_config: ServerConfig,
        room_config: cove_config::EuphRoom,
        vault: EuphRoomVault,
        tz: TimeZone,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        Self {
            config,
            server_config,
            room_config,
            ui_event_tx,
            room: None,
            focus: Focus::Chat,
            state: State::Normal,
            popups: VecDeque::new(),
            chat: ChatState::new(vault, tz),
            last_msg_sent: None,
            nick_list: ListState::new(),
        }
    }

    fn vault(&self) -> &EuphRoomVault {
        self.chat.store()
    }

    fn domain(&self) -> &str {
        &self.vault().room().domain
    }

    fn name(&self) -> &str {
        &self.vault().room().name
    }

    pub fn connect(&mut self, next_instance_id: &mut usize) {
        if self.room.is_none() {
            let room = self.vault().room();
            let instance_config = self
                .server_config
                .clone()
                .room(self.vault().room().name.clone())
                .name(format!("{room:?}-{}", next_instance_id))
                .human(true)
                .username(self.room_config.username.clone())
                .force_username(self.room_config.force_username)
                .password(self.room_config.password.clone());
            *next_instance_id = next_instance_id.wrapping_add(1);

            let tx = self.ui_event_tx.clone();
            self.room = Some(euph::Room::new(
                self.vault().clone(),
                instance_config,
                move |e| {
                    let _ = tx.send(UiEvent::Euph(e));
                },
            ));
        }
    }

    pub fn disconnect(&mut self) {
        self.room = None;
    }

    pub fn room_state(&self) -> Option<&euph::State> {
        if let Some(room) = &self.room {
            Some(room.state())
        } else {
            None
        }
    }

    pub fn room_state_joined(&self) -> Option<&Joined> {
        self.room_state().and_then(|s| s.joined())
    }

    pub fn stopped(&self) -> bool {
        self.room.as_ref().map(|r| r.stopped()).unwrap_or(true)
    }

    pub fn retain(&mut self) {
        if let Some(room) = &self.room {
            if room.stopped() {
                self.room = None;
            }
        }
    }

    pub async fn unseen_msgs_count(&self) -> usize {
        logging_unwrap!(self.vault().unseen_msgs_count().await)
    }

    async fn stabilize_pseudo_msg(&mut self) {
        if let Some(id_rx) = &mut self.last_msg_sent {
            match id_rx.try_recv() {
                Ok(id) => {
                    self.chat.send_successful(id);
                    self.last_msg_sent = None;
                }
                Err(TryRecvError::Empty) => {} // Wait a bit longer
                Err(TryRecvError::Closed) => {
                    self.chat.send_failed();
                    self.last_msg_sent = None;
                }
            }
        }
    }

    fn stabilize_focus(&mut self) {
        if self.room_state_joined().is_none() {
            self.focus = Focus::Chat; // There is no nick list to focus on
        }
    }

    fn stabilize_state(&mut self) {
        let room_state = self.room.as_ref().map(|r| r.state());
        match (&mut self.state, room_state) {
            (
                State::Auth(_),
                Some(euph::State::Connected(
                    _,
                    conn::State::Joining(Joining {
                        bounce: Some(_), ..
                    }),
                )),
            ) => {} // Nothing to see here
            (State::Auth(_), _) => self.state = State::Normal,

            (State::Nick(_), Some(euph::State::Connected(_, conn::State::Joined(_)))) => {}
            (State::Nick(_), _) => self.state = State::Normal,

            (State::Account(account), state) => {
                if !account.stabilize(state) {
                    self.state = State::Normal
                }
            }

            _ => {}
        }
    }

    async fn stabilize(&mut self) {
        self.stabilize_pseudo_msg().await;
        self.stabilize_focus();
        self.stabilize_state();
    }

    pub async fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        self.stabilize().await;

        let room_state = self.room.as_ref().map(|room| room.state());
        let status_widget = self.status_widget(room_state).await;
        let chat = match room_state.and_then(|s| s.joined()) {
            Some(joined) => Self::widget_with_nick_list(
                &mut self.chat,
                status_widget,
                &mut self.nick_list,
                joined,
                self.focus,
            ),
            None => Self::widget_without_nick_list(&mut self.chat, status_widget),
        };

        let mut layers = vec![chat];

        match &mut self.state {
            State::Normal => {}
            State::Auth(editor) => layers.push(auth::widget(editor).desync().boxed_async()),
            State::Nick(editor) => layers.push(nick::widget(editor).desync().boxed_async()),
            State::Account(account) => layers.push(account.widget().desync().boxed_async()),
            State::Links(links) => layers.push(links.widget().desync().boxed_async()),
            State::InspectMessage(message) => {
                layers.push(inspect::message_widget(message).desync().boxed_async())
            }
            State::InspectSession(session) => {
                layers.push(inspect::session_widget(session).desync().boxed_async())
            }
        }

        for popup in &self.popups {
            layers.push(popup.widget().desync().boxed_async());
        }

        Layer::new(layers).boxed_async()
    }

    fn widget_without_nick_list(
        chat: &mut EuphChatState,
        status_widget: impl Widget<UiError> + Send + Sync + 'static,
    ) -> BoxedAsync<'_, UiError> {
        let chat_widget = chat.widget(String::new(), true);

        Join2::vertical(
            status_widget.desync().segment().with_fixed(true),
            chat_widget.segment(),
        )
        .boxed_async()
    }

    fn widget_with_nick_list<'a>(
        chat: &'a mut EuphChatState,
        status_widget: impl Widget<UiError> + Send + Sync + 'static,
        nick_list: &'a mut ListState<SessionId>,
        joined: &Joined,
        focus: Focus,
    ) -> BoxedAsync<'a, UiError> {
        let nick_list_widget = nick_list::widget(nick_list, joined, focus == Focus::NickList)
            .padding()
            .with_right(1)
            .border()
            .desync();

        let chat_widget = chat.widget(joined.session.name.clone(), focus == Focus::Chat);

        Join2::horizontal(
            Join2::vertical(
                status_widget.desync().segment().with_fixed(true),
                chat_widget.segment(),
            )
            .segment(),
            nick_list_widget.segment().with_fixed(true),
        )
        .boxed_async()
    }

    async fn status_widget(&self, state: Option<&euph::State>) -> impl Widget<UiError> {
        let room_style = Style::new().bold().blue();
        let mut info = Styled::new(format!("{} ", self.domain()), Style::new().grey())
            .then(format!("&{}", self.name()), room_style);

        info = match state {
            None | Some(euph::State::Stopped) => info.then_plain(", archive"),
            Some(euph::State::Disconnected) => info.then_plain(", waiting..."),
            Some(euph::State::Connecting) => info.then_plain(", connecting..."),
            Some(euph::State::Connected(_, conn::State::Joining(j))) if j.bounce.is_some() => {
                info.then_plain(", auth required")
            }
            Some(euph::State::Connected(_, conn::State::Joining(_))) => {
                info.then_plain(", joining...")
            }
            Some(euph::State::Connected(_, conn::State::Joined(j))) => {
                let nick = &j.session.name;
                if nick.is_empty() {
                    info.then_plain(", present without nick")
                } else {
                    info.then_plain(", present as ")
                        .and_then(euph::style_nick(nick, Style::new()))
                }
            }
        };

        let unseen = self.unseen_msgs_count().await;
        if unseen > 0 {
            info = info
                .then_plain(" (")
                .then(format!("{unseen}"), Style::new().bold().green())
                .then_plain(")");
        }

        let title = if unseen > 0 {
            format!("&{} ({unseen})", self.name())
        } else {
            format!("&{}", self.name())
        };

        Text::new(info)
            .padding()
            .with_horizontal(1)
            .border()
            .title(title)
    }

    async fn handle_chat_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> bool {
        let can_compose = self.room_state_joined().is_some();

        let reaction = self.chat.handle_input_event(event, keys, can_compose).await;
        let reaction = logging_unwrap!(reaction);

        match reaction {
            Reaction::NotHandled => {}
            Reaction::Handled => return true,
            Reaction::Composed { parent, content } => {
                if let Some(room) = &self.room {
                    match room.send(parent, content) {
                        Ok(id_rx) => self.last_msg_sent = Some(id_rx),
                        Err(_) => self.chat.send_failed(),
                    }
                    return true;
                }
            }
        }

        false
    }

    async fn handle_room_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> bool {
        match self.room_state() {
            // Authenticating
            Some(euph::State::Connected(
                _,
                conn::State::Joining(Joining {
                    bounce: Some(_), ..
                }),
            )) => {
                if event.matches(&keys.room.action.authenticate) {
                    self.state = State::Auth(auth::new());
                    return true;
                }
            }

            // Joined
            Some(euph::State::Connected(_, conn::State::Joined(joined))) => {
                if event.matches(&keys.room.action.nick) {
                    self.state = State::Nick(nick::new(joined.clone()));
                    return true;
                }
                if event.matches(&keys.room.action.more_messages) {
                    if let Some(room) = &self.room {
                        let _ = room.log();
                    }
                    return true;
                }
                if event.matches(&keys.room.action.account) {
                    self.state = State::Account(AccountUiState::new());
                    return true;
                }
            }

            // Otherwise
            _ => {}
        }

        false
    }

    async fn handle_chat_focus_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
    ) -> bool {
        // We need to handle chat input first, otherwise the other
        // key bindings will shadow characters in the editor.
        if self.handle_chat_input_event(event, keys).await {
            return true;
        }

        if self.handle_room_input_event(event, keys).await {
            return true;
        }

        if event.matches(&keys.tree.action.inspect) {
            if let Some(id) = self.chat.cursor() {
                if let Some(msg) = logging_unwrap!(self.vault().full_msg(*id).await) {
                    self.state = State::InspectMessage(msg);
                }
            }
            return true;
        }

        if event.matches(&keys.tree.action.links) {
            if let Some(id) = self.chat.cursor() {
                if let Some(msg) = logging_unwrap!(self.vault().msg(*id).await) {
                    self.state = State::Links(LinksState::new(self.config, &msg.content));
                }
            }
            return true;
        }

        false
    }

    fn handle_nick_list_focus_input_event(
        &mut self,
        event: &mut InputEvent<'_>,
        keys: &Keys,
    ) -> bool {
        if util::handle_list_input_event(&mut self.nick_list, event, keys) {
            return true;
        }

        if event.matches(&keys.tree.action.inspect) {
            if let Some(joined) = self.room_state_joined() {
                if let Some(id) = self.nick_list.selected() {
                    if *id == joined.session.session_id {
                        self.state =
                            State::InspectSession(SessionInfo::Full(joined.session.clone()));
                    } else if let Some(session) = joined.listing.get(id) {
                        self.state = State::InspectSession(session.clone());
                    }
                }
            }
            return true;
        }

        false
    }

    async fn handle_normal_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> bool {
        match self.focus {
            Focus::Chat => {
                if self.handle_chat_focus_input_event(event, keys).await {
                    return true;
                }

                if self.room_state_joined().is_some() && event.matches(&keys.general.focus) {
                    self.focus = Focus::NickList;
                    return true;
                }
            }
            Focus::NickList => {
                if event.matches(&keys.general.abort) || event.matches(&keys.general.focus) {
                    self.focus = Focus::Chat;
                    return true;
                }

                if self.handle_nick_list_focus_input_event(event, keys) {
                    return true;
                }
            }
        }

        false
    }

    pub async fn handle_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> bool {
        if !self.popups.is_empty() {
            if event.matches(&keys.general.abort) {
                self.popups.pop_back();
                return true;
            }
            // Prevent event from reaching anything below the popup
            return false;
        }

        let result = match &mut self.state {
            State::Normal => return self.handle_normal_input_event(event, keys).await,
            State::Auth(editor) => auth::handle_input_event(event, keys, &self.room, editor),
            State::Nick(editor) => nick::handle_input_event(event, keys, &self.room, editor),
            State::Account(account) => account.handle_input_event(event, keys, &self.room),
            State::Links(links) => links.handle_input_event(event, keys),
            State::InspectMessage(_) | State::InspectSession(_) => {
                inspect::handle_input_event(event, keys)
            }
        };

        match result {
            PopupResult::NotHandled => false,
            PopupResult::Handled => true,
            PopupResult::Close => {
                self.state = State::Normal;
                true
            }
            PopupResult::ErrorOpeningLink { link, error } => {
                self.popups.push_front(RoomPopup::Error {
                    description: format!("Failed to open link: {link}"),
                    reason: format!("{error}"),
                });
                true
            }
        }
    }

    pub async fn handle_event(&mut self, event: Event) -> bool {
        let Some(room) = &self.room else { return false };

        if event.config().name != room.instance().config().name {
            // If we allowed names other than the current one, old instances
            // that haven't yet shut down properly could mess up our state.
            return false;
        }

        // We handle the packet internally first because the room event handling
        // will consume it while we only need a reference.
        let handled = if let Event::Packet(_, packet, _) = &event {
            match &packet.content {
                Ok(data) => self.handle_euph_data(data),
                Err(reason) => self.handle_euph_error(packet.r#type, reason),
            }
        } else {
            // The room state changes, which always means a redraw.
            true
        };

        self.room
            .as_mut()
            // See check at the beginning of the function.
            .expect("no room even though we checked earlier")
            .handle_event(event)
            .await;

        handled
    }

    fn handle_euph_data(&mut self, data: &Data) -> bool {
        // These packets don't result in any noticeable change in the UI.
        #[allow(clippy::match_like_matches_macro)]
        let handled = match data {
            Data::PingEvent(_) | Data::PingReply(_) => {
                // Pings are displayed nowhere in the room UI.
                false
            }
            Data::DisconnectEvent(_) => {
                // Followed by the server closing the connection, meaning that
                // we'll get an `EuphRoomEvent::Disconnected` soon after this.
                false
            }
            _ => true,
        };

        // Because the euphoria API is very carefully designed with emphasis on
        // consistency, some failures are not normal errors but instead
        // error-free replies that encode their own error.
        let error = match data {
            Data::AuthReply(reply) if !reply.success => {
                Some(("authenticate", reply.reason.clone()))
            }
            Data::LoginReply(reply) if !reply.success => Some(("login", reply.reason.clone())),
            _ => None,
        };
        if let Some((action, reason)) = error {
            let description = format!("Failed to {action}.");
            let reason = reason.unwrap_or_else(|| "no idea, the server wouldn't say".to_string());
            self.popups.push_front(RoomPopup::Error {
                description,
                reason,
            });
        }

        handled
    }

    fn handle_euph_error(&mut self, r#type: PacketType, reason: &str) -> bool {
        let action = match r#type {
            PacketType::AuthReply => "authenticate",
            PacketType::NickReply => "set nick",
            PacketType::PmInitiateReply => "initiate pm",
            PacketType::SendReply => "send message",
            PacketType::ChangeEmailReply => "change account email",
            PacketType::ChangeNameReply => "change account name",
            PacketType::ChangePasswordReply => "change account password",
            PacketType::LoginReply => "log in",
            PacketType::LogoutReply => "log out",
            PacketType::RegisterAccountReply => "register account",
            PacketType::ResendVerificationEmailReply => "resend verification email",
            PacketType::ResetPasswordReply => "reset account password",
            PacketType::BanReply => "ban",
            PacketType::EditMessageReply => "edit message",
            PacketType::GrantAccessReply => "grant room access",
            PacketType::GrantManagerReply => "grant manager permissions",
            PacketType::RevokeAccessReply => "revoke room access",
            PacketType::RevokeManagerReply => "revoke manager permissions",
            PacketType::UnbanReply => "unban",
            _ => return false,
        };
        let description = format!("Failed to {action}.");
        self.popups.push_front(RoomPopup::Error {
            description,
            reason: reason.to_string(),
        });
        true
    }
}
