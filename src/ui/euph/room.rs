use std::collections::VecDeque;
use std::sync::Arc;

use crossterm::style::Stylize;
use euphoxide::api::{Data, Message, MessageId, PacketType, SessionId};
use euphoxide::bot::instance::{Event, ServerConfig};
use euphoxide::conn::{self, Joined, Joining, SessionInfo};
use parking_lot::FairMutex;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use toss::widgets::{BoxedAsync, Join2, Layer, Text};
use toss::{AsyncWidget, Style, Styled, Terminal, WidgetExt};

use crate::config;
use crate::euph;
use crate::macros::logging_unwrap;
use crate::ui::chat::{ChatState, Reaction};
use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::widgets::editor::EditorState as OldEditorState;
use crate::ui::widgets::WidgetWrapper;
use crate::ui::widgets2::ListState;
use crate::ui::{util2, UiError, UiEvent};
use crate::vault::EuphRoomVault;

use super::account::{self, AccountUiState};
use super::links::{self, LinksState};
use super::popup::RoomPopup;
use super::{auth, inspect, nick, nick_list};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Chat,
    NickList,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Normal,
    Auth(OldEditorState),
    Nick(OldEditorState),
    Account(AccountUiState),
    Links(LinksState),
    InspectMessage(Message),
    InspectSession(SessionInfo),
}

type EuphChatState = ChatState<euph::SmallMessage, EuphRoomVault>;

pub struct EuphRoom {
    server_config: ServerConfig,
    config: config::EuphRoom,
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
        server_config: ServerConfig,
        config: config::EuphRoom,
        vault: EuphRoomVault,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        Self {
            server_config,
            config,
            ui_event_tx,
            room: None,
            focus: Focus::Chat,
            state: State::Normal,
            popups: VecDeque::new(),
            chat: ChatState::new(vault),
            last_msg_sent: None,
            nick_list: ListState::new(),
        }
    }

    fn vault(&self) -> &EuphRoomVault {
        self.chat.store()
    }

    fn name(&self) -> &str {
        self.vault().room()
    }

    pub fn connect(&mut self, next_instance_id: &mut usize) {
        if self.room.is_none() {
            let room = self.vault().room();
            let instance_config = self
                .server_config
                .clone()
                .room(self.vault().room().to_string())
                .name(format!("{room}-{}", next_instance_id))
                .human(true)
                .username(self.config.username.clone())
                .force_username(self.config.force_username)
                .password(self.config.password.clone());
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

    // TODO fn room_state_joined(&self) -> Option<&Joined> {}

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
                    self.chat.sent(Some(id)).await;
                    self.last_msg_sent = None;
                }
                Err(TryRecvError::Empty) => {} // Wait a bit longer
                Err(TryRecvError::Closed) => {
                    self.chat.sent(None).await;
                    self.last_msg_sent = None;
                }
            }
        }
    }

    fn stabilize_focus(&mut self) {
        match self.room_state() {
            Some(euph::State::Connected(_, conn::State::Joined(_))) => {}
            _ => self.focus = Focus::Chat, // There is no nick list to focus on
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
        let chat = if let Some(euph::State::Connected(_, conn::State::Joined(joined))) = room_state
        {
            Self::widget_with_nick_list(
                &mut self.chat,
                status_widget,
                &mut self.nick_list,
                joined,
                self.focus,
            )
        } else {
            Self::widget_without_nick_list(&mut self.chat, status_widget)
        };

        let mut layers = vec![chat];

        match &self.state {
            State::Normal => {}
            State::Auth(editor) => {
                layers.push(WidgetWrapper::new(auth::widget(editor)).boxed_async())
            }
            State::Nick(editor) => {
                layers.push(WidgetWrapper::new(nick::widget(editor)).boxed_async())
            }
            State::Account(account) => {
                layers.push(WidgetWrapper::new(account.widget()).boxed_async())
            }
            State::Links(links) => layers.push(WidgetWrapper::new(links.widget()).boxed_async()),
            State::InspectMessage(message) => {
                layers.push(WidgetWrapper::new(inspect::message_widget(message)).boxed_async())
            }
            State::InspectSession(session) => {
                layers.push(WidgetWrapper::new(inspect::session_widget(session)).boxed_async())
            }
        }

        for popup in &self.popups {
            layers.push(WidgetWrapper::new(popup.widget()).boxed_async());
        }

        Layer::new(layers).boxed_async()
    }

    fn widget_without_nick_list(
        chat: &mut EuphChatState,
        status_widget: impl AsyncWidget<UiError> + Send + Sync + 'static,
    ) -> BoxedAsync<'_, UiError> {
        let chat_widget = WidgetWrapper::new(chat.widget(String::new(), true));

        Join2::vertical(
            status_widget.segment().with_fixed(true),
            chat_widget.segment(),
        )
        .boxed_async()
    }

    fn widget_with_nick_list<'a>(
        chat: &'a mut EuphChatState,
        status_widget: impl AsyncWidget<UiError> + Send + Sync + 'static,
        nick_list: &'a mut ListState<SessionId>,
        joined: &Joined,
        focus: Focus,
    ) -> BoxedAsync<'a, UiError> {
        let nick_list_widget = nick_list::widget(nick_list, joined, focus == Focus::NickList)
            .padding()
            .with_right(1)
            .border();

        let chat_widget =
            WidgetWrapper::new(chat.widget(joined.session.name.clone(), focus == Focus::Chat));

        Join2::horizontal(
            Join2::vertical(
                status_widget.segment().with_fixed(true),
                chat_widget.segment(),
            )
            .segment(),
            nick_list_widget.segment().with_fixed(true),
        )
        .boxed_async()
    }

    async fn status_widget(&self, state: Option<&euph::State>) -> BoxedAsync<'static, UiError> {
        let room_style = Style::new().bold().blue();
        let mut info = Styled::new(format!("&{}", self.name()), room_style);

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

        Text::new(info)
            .padding()
            .with_horizontal(1)
            .border()
            .boxed_async()
    }

    async fn list_chat_key_bindings(&self, bindings: &mut KeyBindingsList) {
        let can_compose = matches!(
            self.room_state(),
            Some(euph::State::Connected(_, conn::State::Joined(_)))
        );
        self.chat.list_key_bindings(bindings, can_compose).await;
    }

    async fn handle_chat_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
    ) -> bool {
        let can_compose = matches!(
            self.room_state(),
            Some(euph::State::Connected(_, conn::State::Joined(_)))
        );

        let reaction = self
            .chat
            .handle_input_event(terminal, crossterm_lock, event, can_compose)
            .await;
        let reaction = logging_unwrap!(reaction);

        match reaction {
            Reaction::NotHandled => {}
            Reaction::Handled => return true,
            Reaction::Composed { parent, content } => {
                if let Some(room) = &self.room {
                    match room.send(parent, content) {
                        Ok(id_rx) => self.last_msg_sent = Some(id_rx),
                        Err(_) => self.chat.sent(None).await,
                    }
                    return true;
                }
            }
            Reaction::ComposeError(e) => {
                self.popups.push_front(RoomPopup::Error {
                    description: "Failed to use external editor".to_string(),
                    reason: format!("{e}"),
                });
                return true;
            }
        }

        false
    }

    fn list_room_key_bindings(&self, bindings: &mut KeyBindingsList) {
        match self.room_state() {
            // Authenticating
            Some(euph::State::Connected(
                _,
                conn::State::Joining(Joining {
                    bounce: Some(_), ..
                }),
            )) => {
                bindings.binding("a", "authenticate");
            }

            // Connected
            Some(euph::State::Connected(_, conn::State::Joined(_))) => {
                bindings.binding("n", "change nick");
                bindings.binding("m", "download more messages");
                bindings.binding("A", "show account ui");
            }

            // Otherwise
            _ => {}
        }

        // Inspecting messages
        bindings.binding("i", "inspect message");
        bindings.binding("I", "show message links");
        bindings.binding("ctrl+p", "open room's plugh.de/present page");
    }

    async fn handle_room_input_event(&mut self, event: &InputEvent) -> bool {
        match self.room_state() {
            // Authenticating
            Some(euph::State::Connected(
                _,
                conn::State::Joining(Joining {
                    bounce: Some(_), ..
                }),
            )) => {
                if let key!('a') = event {
                    self.state = State::Auth(auth::new());
                    return true;
                }
            }

            // Joined
            Some(euph::State::Connected(_, conn::State::Joined(joined))) => match event {
                key!('n') | key!('N') => {
                    self.state = State::Nick(nick::new(joined.clone()));
                    return true;
                }
                key!('m') => {
                    if let Some(room) = &self.room {
                        let _ = room.log();
                    }
                    return true;
                }
                key!('A') => {
                    self.state = State::Account(AccountUiState::new());
                    return true;
                }
                _ => {}
            },

            // Otherwise
            _ => {}
        }

        // Always applicable
        match event {
            key!('i') => {
                if let Some(id) = self.chat.cursor().await {
                    if let Some(msg) = logging_unwrap!(self.vault().full_msg(id).await) {
                        self.state = State::InspectMessage(msg);
                    }
                }
                return true;
            }
            key!('I') => {
                if let Some(id) = self.chat.cursor().await {
                    if let Some(msg) = logging_unwrap!(self.vault().msg(id).await) {
                        self.state = State::Links(LinksState::new(&msg.content));
                    }
                }
                return true;
            }
            key!(Ctrl + 'p') => {
                let link = format!("https://plugh.de/present/{}/", self.name());
                if let Err(error) = open::that(&link) {
                    self.popups.push_front(RoomPopup::Error {
                        description: format!("Failed to open link: {link}"),
                        reason: format!("{error}"),
                    });
                }
                return true;
            }
            _ => {}
        }

        false
    }

    async fn list_chat_focus_key_bindings(&self, bindings: &mut KeyBindingsList) {
        self.list_room_key_bindings(bindings);
        bindings.empty();
        self.list_chat_key_bindings(bindings).await;
    }

    async fn handle_chat_focus_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
    ) -> bool {
        // We need to handle chat input first, otherwise the other
        // key bindings will shadow characters in the editor.
        if self
            .handle_chat_input_event(terminal, crossterm_lock, event)
            .await
        {
            return true;
        }

        if self.handle_room_input_event(event).await {
            return true;
        }

        false
    }

    fn list_nick_list_focus_key_bindings(&self, bindings: &mut KeyBindingsList) {
        util2::list_list_key_bindings(bindings);

        bindings.binding("i", "inspect session");
    }

    fn handle_nick_list_focus_input_event(&mut self, event: &InputEvent) -> bool {
        if util2::handle_list_input_event(&mut self.nick_list, event) {
            return true;
        }

        if let key!('i') = event {
            if let Some(euph::State::Connected(_, conn::State::Joined(joined))) = self.room_state()
            {
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

    pub async fn list_normal_key_bindings(&self, bindings: &mut KeyBindingsList) {
        // Handled in rooms list, not here
        bindings.binding("esc", "leave room");

        match self.focus {
            Focus::Chat => {
                if let Some(euph::State::Connected(_, conn::State::Joined(_))) = self.room_state() {
                    bindings.binding("tab", "focus on nick list");
                }

                self.list_chat_focus_key_bindings(bindings).await;
            }
            Focus::NickList => {
                bindings.binding("tab, esc", "focus on chat");
                bindings.empty();
                bindings.heading("Nick list");
                self.list_nick_list_focus_key_bindings(bindings);
            }
        }
    }

    async fn handle_normal_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
    ) -> bool {
        match self.focus {
            Focus::Chat => {
                // Needs to be handled first or the tab key may be shadowed
                // during editing.
                if self
                    .handle_chat_focus_input_event(terminal, crossterm_lock, event)
                    .await
                {
                    return true;
                }

                if let Some(euph::State::Connected(_, conn::State::Joined(_))) = self.room_state() {
                    if let key!(Tab) = event {
                        self.focus = Focus::NickList;
                        return true;
                    }
                }
            }
            Focus::NickList => {
                if let key!(Tab) | key!(Esc) = event {
                    self.focus = Focus::Chat;
                    return true;
                }

                if self.handle_nick_list_focus_input_event(event) {
                    return true;
                }
            }
        }

        false
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.heading("Room");

        if !self.popups.is_empty() {
            bindings.binding("esc", "close popup");
            return;
        }

        match &self.state {
            State::Normal => self.list_normal_key_bindings(bindings).await,
            State::Auth(_) => auth::list_key_bindings(bindings),
            State::Nick(_) => nick::list_key_bindings(bindings),
            State::Account(account) => account.list_key_bindings(bindings),
            State::Links(links) => links.list_key_bindings(bindings),
            State::InspectMessage(_) | State::InspectSession(_) => {
                inspect::list_key_bindings(bindings)
            }
        }
    }

    pub async fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
    ) -> bool {
        if !self.popups.is_empty() {
            if matches!(event, key!(Esc)) {
                self.popups.pop_back();
                return true;
            }
            return false;
        }

        // TODO Use a common EventResult

        match &mut self.state {
            State::Normal => {
                self.handle_normal_input_event(terminal, crossterm_lock, event)
                    .await
            }
            State::Auth(editor) => {
                match auth::handle_input_event(terminal, event, &self.room, editor) {
                    auth::EventResult::NotHandled => false,
                    auth::EventResult::Handled => true,
                    auth::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
            State::Nick(editor) => {
                match nick::handle_input_event(terminal, event, &self.room, editor) {
                    nick::EventResult::NotHandled => false,
                    nick::EventResult::Handled => true,
                    nick::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
            State::Account(account) => {
                match account.handle_input_event(terminal, event, &self.room) {
                    account::EventResult::NotHandled => false,
                    account::EventResult::Handled => true,
                    account::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
            State::Links(links) => match links.handle_input_event(event) {
                links::EventResult::NotHandled => false,
                links::EventResult::Handled => true,
                links::EventResult::Close => {
                    self.state = State::Normal;
                    true
                }
                links::EventResult::ErrorOpeningLink { link, error } => {
                    self.popups.push_front(RoomPopup::Error {
                        description: format!("Failed to open link: {link}"),
                        reason: format!("{error}"),
                    });
                    true
                }
            },
            State::InspectMessage(_) | State::InspectSession(_) => {
                match inspect::handle_input_event(event) {
                    inspect::EventResult::NotHandled => false,
                    inspect::EventResult::Close => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
        }
    }

    pub async fn handle_event(&mut self, event: Event) -> bool {
        let room = match &self.room {
            None => return false,
            Some(room) => room,
        };

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
