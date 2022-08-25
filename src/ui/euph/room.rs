use std::collections::VecDeque;
use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use euphoxide::api::{Data, PacketType, Snowflake};
use euphoxide::conn::{Joined, Joining, Status};
use parking_lot::FairMutex;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::config;
use crate::euph::{self, EuphRoomEvent};
use crate::macros::{ok_or_return, some_or_return};
use crate::store::MsgStore;
use crate::ui::chat::{ChatState, Reaction};
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::widgets::border::Border;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::join::{HJoin, Segment, VJoin};
use crate::ui::widgets::layer::Layer;
use crate::ui::widgets::list::ListState;
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;
use crate::ui::UiEvent;
use crate::vault::EuphVault;

use super::account::{self, AccountUiState};
use super::popup::RoomPopup;
use super::{auth, nick, nick_list};

enum State {
    Normal,
    Auth(EditorState),
    Nick(EditorState),
    Account(AccountUiState),
}

#[allow(clippy::large_enum_variant)]
pub enum RoomStatus {
    NoRoom,
    Stopped,
    Connecting,
    Connected(Status),
}

pub struct EuphRoom {
    config: config::EuphRoom,

    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    vault: EuphVault,
    room: Option<euph::Room>,

    state: State,
    popups: VecDeque<RoomPopup>,

    chat: ChatState<euph::SmallMessage, EuphVault>,
    last_msg_sent: Option<oneshot::Receiver<Snowflake>>,

    nick_list: ListState<String>,
}

impl EuphRoom {
    pub fn new(
        config: config::EuphRoom,
        vault: EuphVault,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) -> Self {
        Self {
            config,
            ui_event_tx,
            vault: vault.clone(),
            room: None,
            state: State::Normal,
            popups: VecDeque::new(),
            chat: ChatState::new(vault),
            last_msg_sent: None,
            nick_list: ListState::new(),
        }
    }

    async fn shovel_room_events(
        name: String,
        mut euph_room_event_rx: mpsc::UnboundedReceiver<EuphRoomEvent>,
        ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    ) {
        loop {
            let event = some_or_return!(euph_room_event_rx.recv().await);
            let event = UiEvent::EuphRoom {
                name: name.clone(),
                event,
            };
            ok_or_return!(ui_event_tx.send(event));
        }
    }

    pub fn connect(&mut self) {
        if self.room.is_none() {
            let store = self.chat.store().clone();
            let name = store.room().to_string();
            let (room, euph_room_event_rx) = euph::Room::new(
                store,
                self.config.username.clone(),
                self.config.force_username,
                self.config.password.clone(),
            );

            self.room = Some(room);

            tokio::task::spawn(Self::shovel_room_events(
                name,
                euph_room_event_rx,
                self.ui_event_tx.clone(),
            ));
        }
    }

    pub fn disconnect(&mut self) {
        self.room = None;
    }

    pub async fn status(&self) -> RoomStatus {
        match &self.room {
            Some(room) => match room.status().await {
                Ok(Some(status)) => RoomStatus::Connected(status),
                Ok(None) => RoomStatus::Connecting,
                Err(_) => RoomStatus::Stopped,
            },
            None => RoomStatus::NoRoom,
        }
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
        self.vault.unseen_msgs_count().await
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

    fn stabilize_state(&mut self, status: &RoomStatus) {
        match &mut self.state {
            State::Auth(_)
                if !matches!(
                    status,
                    RoomStatus::Connected(Status::Joining(Joining {
                        bounce: Some(_),
                        ..
                    }))
                ) =>
            {
                self.state = State::Normal
            }
            State::Nick(_) if !matches!(status, RoomStatus::Connected(Status::Joined(_))) => {
                self.state = State::Normal
            }
            State::Account(account) => {
                if !account.stabilize(status) {
                    self.state = State::Normal
                }
            }
            _ => {}
        }
    }

    async fn stabilize(&mut self, status: &RoomStatus) {
        self.stabilize_pseudo_msg().await;
        self.stabilize_state(status);
    }

    pub async fn widget(&mut self) -> BoxedWidget {
        let status = self.status().await;
        self.stabilize(&status).await;

        let chat = if let RoomStatus::Connected(Status::Joined(joined)) = &status {
            self.widget_with_nick_list(&status, joined).await
        } else {
            self.widget_without_nick_list(&status).await
        };

        let mut layers = vec![chat];

        match &self.state {
            State::Normal => {}
            State::Auth(editor) => layers.push(auth::widget(editor)),
            State::Nick(editor) => layers.push(nick::widget(editor)),
            State::Account(account) => layers.push(account.widget()),
        }

        for popup in &self.popups {
            layers.push(popup.widget());
        }

        Layer::new(layers).into()
    }

    async fn widget_without_nick_list(&self, status: &RoomStatus) -> BoxedWidget {
        VJoin::new(vec![
            Segment::new(Border::new(
                Padding::new(self.status_widget(status).await).horizontal(1),
            )),
            // TODO Use last known nick?
            Segment::new(self.chat.widget(String::new())).expanding(true),
        ])
        .into()
    }

    async fn widget_with_nick_list(&self, status: &RoomStatus, joined: &Joined) -> BoxedWidget {
        HJoin::new(vec![
            Segment::new(VJoin::new(vec![
                Segment::new(Border::new(
                    Padding::new(self.status_widget(status).await).horizontal(1),
                )),
                Segment::new(self.chat.widget(joined.session.name.clone())).expanding(true),
            ]))
            .expanding(true),
            Segment::new(Border::new(
                Padding::new(nick_list::widget(&self.nick_list, joined)).right(1),
            )),
        ])
        .into()
    }

    async fn status_widget(&self, status: &RoomStatus) -> BoxedWidget {
        // TODO Include unread message count
        let room = self.chat.store().room();
        let room_style = ContentStyle::default().bold().blue();
        let mut info = Styled::new(format!("&{room}"), room_style);

        info = match status {
            RoomStatus::NoRoom | RoomStatus::Stopped => info.then_plain(", archive"),
            RoomStatus::Connecting => info.then_plain(", connecting..."),
            RoomStatus::Connected(Status::Joining(j)) if j.bounce.is_some() => {
                info.then_plain(", auth required")
            }
            RoomStatus::Connected(Status::Joining(_)) => info.then_plain(", joining..."),
            RoomStatus::Connected(Status::Joined(j)) => {
                let nick = &j.session.name;
                if nick.is_empty() {
                    info.then_plain(", present without nick")
                } else {
                    let nick_style = euph::nick_style(nick);
                    info.then_plain(", present as ").then(nick, nick_style)
                }
            }
        };

        let unseen = self.unseen_msgs_count().await;
        if unseen > 0 {
            info = info
                .then_plain(" (")
                .then(format!("{unseen}"), ContentStyle::default().bold().green())
                .then_plain(")");
        }

        Text::new(info).into()
    }

    pub async fn list_normal_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "leave room");

        let can_compose = if let Some(room) = &self.room {
            match room.status().await.ok().flatten() {
                Some(Status::Joining(Joining {
                    bounce: Some(_), ..
                })) => {
                    bindings.binding("a", "authenticate");
                    false
                }
                Some(Status::Joined(_)) => {
                    bindings.binding("n", "change nick");
                    bindings.binding("m", "download more messages");
                    bindings.binding("A", "show account ui");
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        bindings.empty();
        self.chat.list_key_bindings(bindings, can_compose).await;
    }

    async fn handle_normal_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
    ) -> bool {
        if let Some(room) = &self.room {
            let status = room.status().await;
            let can_compose = matches!(status, Ok(Some(Status::Joined(_))));

            // We need to handle chat input first, otherwise the other
            // key bindings will shadow characters in the editor.
            match self
                .chat
                .handle_input_event(terminal, crossterm_lock, event, can_compose)
                .await
            {
                Reaction::NotHandled => {}
                Reaction::Handled => return true,
                Reaction::Composed { parent, content } => {
                    match room.send(parent, content) {
                        Ok(id_rx) => self.last_msg_sent = Some(id_rx),
                        Err(_) => self.chat.sent(None).await,
                    }
                    return true;
                }
            }

            match status.ok().flatten() {
                Some(Status::Joining(Joining {
                    bounce: Some(_), ..
                })) if matches!(event, key!('a')) => {
                    self.state = State::Auth(auth::new());
                    true
                }
                Some(Status::Joined(joined)) => match event {
                    key!('n') | key!('N') => {
                        self.state = State::Nick(nick::new(joined));
                        true
                    }
                    key!('m') => {
                        if let Some(room) = &self.room {
                            let _ = room.log();
                        }
                        true
                    }
                    key!('A') => {
                        self.state = State::Account(AccountUiState::new());
                        true
                    }
                    _ => false,
                },
                _ => false,
            }
        } else {
            self.chat
                .handle_input_event(terminal, crossterm_lock, event, false)
                .await
                .handled()
        }
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

        match &mut self.state {
            State::Normal => {
                self.handle_normal_input_event(terminal, crossterm_lock, event)
                    .await
            }
            State::Auth(editor) => {
                match auth::handle_input_event(terminal, crossterm_lock, event, &self.room, editor)
                {
                    auth::EventResult::NotHandled => false,
                    auth::EventResult::Handled => true,
                    auth::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
            State::Nick(editor) => {
                match nick::handle_input_event(terminal, crossterm_lock, event, &self.room, editor)
                {
                    nick::EventResult::NotHandled => false,
                    nick::EventResult::Handled => true,
                    nick::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
            State::Account(account) => {
                match account.handle_input_event(terminal, crossterm_lock, event, &self.room) {
                    account::EventResult::NotHandled => false,
                    account::EventResult::Handled => true,
                    account::EventResult::ResetState => {
                        self.state = State::Normal;
                        true
                    }
                }
            }
        }
    }

    pub fn handle_euph_room_event(&mut self, event: EuphRoomEvent) -> bool {
        match event {
            EuphRoomEvent::Connected | EuphRoomEvent::Disconnected | EuphRoomEvent::Stopped => true,
            EuphRoomEvent::Packet(packet) => match packet.content {
                Ok(data) => self.handle_euph_data(data),
                Err(reason) => self.handle_euph_error(packet.r#type, reason),
            },
        }
    }

    fn handle_euph_data(&mut self, data: Data) -> bool {
        // These packets don't result in any noticeable change in the UI.
        #[allow(clippy::match_like_matches_macro)]
        let handled = match &data {
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
            Data::AuthReply(reply) if !reply.success => Some(("authenticate", reply.reason)),
            Data::LoginReply(reply) if !reply.success => Some(("login", reply.reason)),
            _ => None,
        };
        if let Some((action, reason)) = error {
            let description = format!("Failed to {action}.");
            let reason = reason.unwrap_or_else(|| "no idea, the server wouldn't say".to_string());
            self.popups.push_front(RoomPopup::ServerError {
                description,
                reason,
            });
        }

        handled
    }

    fn handle_euph_error(&mut self, r#type: PacketType, reason: String) -> bool {
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
        self.popups.push_front(RoomPopup::ServerError {
            description,
            reason,
        });
        true
    }
}
