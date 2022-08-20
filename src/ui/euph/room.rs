use std::collections::VecDeque;
use std::iter;
use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{Color, ContentStyle, Stylize};
use euphoxide::api::{Data, PacketType, SessionType, SessionView, Snowflake};
use euphoxide::conn::{Joined, Joining, Status};
use parking_lot::FairMutex;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::{self, EuphRoomEvent};
use crate::macros::{ok_or_return, some_or_return};
use crate::store::MsgStore;
use crate::ui::chat::{ChatState, Reaction};
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::widgets::background::Background;
use crate::ui::widgets::border::Border;
use crate::ui::widgets::cursor::Cursor;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::join::{HJoin, Segment, VJoin};
use crate::ui::widgets::layer::Layer;
use crate::ui::widgets::list::{List, ListState};
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;
use crate::ui::{util, UiEvent};
use crate::vault::EuphVault;

use super::popup::RoomPopup;

enum State {
    Normal,
    Auth(EditorState),
    Nick(EditorState),
}

#[allow(clippy::large_enum_variant)]
pub enum RoomStatus {
    NoRoom,
    Stopped,
    Connecting,
    Connected(Status),
}

pub struct EuphRoom {
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
    pub fn new(vault: EuphVault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
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
            let (room, euph_room_event_rx) = euph::Room::new(store);

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
        match &self.state {
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
            State::Auth(_) => layers.push(Self::auth_widget()),
            State::Nick(editor) => layers.push(Self::nick_widget(editor)),
        }

        for popup in &self.popups {
            layers.push(popup.widget());
        }

        Layer::new(layers).into()
    }

    fn auth_widget() -> BoxedWidget {
        Popup::new(
            Padding::new(Cursor::new(Text::new((
                "<hidden>",
                ContentStyle::default().grey().italic(),
            ))))
            .left(1),
        )
        .title("Enter password")
        .inner_padding(false)
        .build()
    }

    fn nick_widget(editor: &EditorState) -> BoxedWidget {
        let editor = editor
            .widget()
            .highlight(|s| Styled::new(s, euph::nick_style(s)));
        Popup::new(Padding::new(editor).left(1))
            .title("Choose nick")
            .inner_padding(false)
            .build()
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
                Padding::new(self.nick_list_widget(joined)).right(1),
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

    fn render_nick_list_row(
        list: &mut List<String>,
        session: &SessionView,
        own_session: &SessionView,
    ) {
        let id = session.session_id.clone();

        let (name, style, style_inv) = if session.name.is_empty() {
            let name = "lurk";
            let style = ContentStyle::default().grey();
            let style_inv = ContentStyle::default().black().on_grey();
            (name, style, style_inv)
        } else {
            let name = &session.name as &str;
            let (r, g, b) = euph::nick_color(name);
            let color = Color::Rgb { r, g, b };
            let style = ContentStyle::default().bold().with(color);
            let style_inv = ContentStyle::default().bold().black().on(color);
            (name, style, style_inv)
        };

        let perms = if session.is_staff {
            "!"
        } else if session.is_manager {
            "*"
        } else if session.id.session_type() == Some(SessionType::Account) {
            "~"
        } else {
            ""
        };

        let owner = if session.session_id == own_session.session_id {
            ">"
        } else {
            " "
        };

        let normal = Styled::new_plain(owner).then(name, style).then_plain(perms);
        let selected = Styled::new_plain(owner)
            .then(name, style_inv)
            .then_plain(perms);
        list.add_sel(
            id,
            Text::new(normal),
            Background::new(Text::new(selected)).style(style_inv),
        );
    }

    fn render_nick_list_section(
        list: &mut List<String>,
        name: &str,
        sessions: &[&SessionView],
        own_session: &SessionView,
    ) {
        if sessions.is_empty() {
            return;
        }

        let heading_style = ContentStyle::new().bold();

        if !list.is_empty() {
            list.add_unsel(Empty::new());
        }

        let row = Styled::new_plain(" ")
            .then(name, heading_style)
            .then_plain(format!(" ({})", sessions.len()));
        list.add_unsel(Text::new(row));

        for session in sessions {
            Self::render_nick_list_row(list, session, own_session);
        }
    }

    fn render_nick_list_rows(list: &mut List<String>, joined: &Joined) {
        let mut people = vec![];
        let mut bots = vec![];
        let mut lurkers = vec![];
        let mut nurkers = vec![];

        let mut sessions = iter::once(&joined.session)
            .chain(joined.listing.values())
            .collect::<Vec<_>>();
        sessions.sort_unstable_by_key(|s| &s.name);
        for sess in sessions {
            match sess.id.session_type() {
                Some(SessionType::Bot) if sess.name.is_empty() => nurkers.push(sess),
                Some(SessionType::Bot) => bots.push(sess),
                _ if sess.name.is_empty() => lurkers.push(sess),
                _ => people.push(sess),
            }
        }

        people.sort_unstable_by_key(|s| (&s.name, &s.session_id));
        bots.sort_unstable_by_key(|s| (&s.name, &s.session_id));
        lurkers.sort_unstable_by_key(|s| &s.session_id);
        nurkers.sort_unstable_by_key(|s| &s.session_id);

        Self::render_nick_list_section(list, "People", &people, &joined.session);
        Self::render_nick_list_section(list, "Bots", &bots, &joined.session);
        Self::render_nick_list_section(list, "Lurkers", &lurkers, &joined.session);
        Self::render_nick_list_section(list, "Nurkers", &nurkers, &joined.session);
    }

    fn nick_list_widget(&self, joined: &Joined) -> BoxedWidget {
        let mut list = self.nick_list.widget();
        Self::render_nick_list_rows(&mut list, joined);
        list.into()
    }

    fn nick_char(c: char) -> bool {
        c != '\n'
    }

    pub async fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.heading("Room");

        if !self.popups.is_empty() {
            bindings.binding("esc", "close popup");
            return;
        }

        match &self.state {
            State::Normal => {
                bindings.binding("esc", "leave room");

                let can_compose = if let Some(room) = &self.room {
                    match room.status().await {
                        Ok(Some(Status::Joining(Joining {
                            bounce: Some(_), ..
                        }))) => {
                            bindings.binding("a", "authenticate");
                            false
                        }
                        Ok(Some(Status::Joined(_))) => {
                            bindings.binding("n", "change nick");
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
            State::Auth(_) => {
                bindings.binding("esc", "abort");
                bindings.binding("enter", "authenticate");
                util::list_editor_key_bindings(bindings, Self::nick_char, false);
            }
            State::Nick(_) => {
                bindings.binding("esc", "abort");
                bindings.binding("enter", "set nick");
                util::list_editor_key_bindings(bindings, Self::nick_char, false);
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

        match &self.state {
            State::Normal => {
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

                    match status {
                        Ok(Some(Status::Joining(Joining {
                            bounce: Some(_), ..
                        }))) => {
                            if let key!('a') | key!('A') = event {
                                self.state = State::Auth(EditorState::new());
                                return true;
                            }
                            false
                        }
                        Ok(Some(Status::Joined(joined))) => {
                            if let key!('n') | key!('N') = event {
                                self.state = State::Nick(EditorState::with_initial_text(
                                    joined.session.name,
                                ));
                                return true;
                            }
                            true
                        }
                        _ => false,
                    }
                } else {
                    self.chat
                        .handle_input_event(terminal, crossterm_lock, event, false)
                        .await
                        .handled()
                }
            }
            State::Auth(ed) => match event {
                key!(Esc) => {
                    self.state = State::Normal;
                    true
                }
                key!(Enter) => {
                    if let Some(room) = &self.room {
                        let _ = room.auth(ed.text());
                    }
                    self.state = State::Normal;
                    true
                }
                _ => util::handle_editor_input_event(
                    ed,
                    terminal,
                    crossterm_lock,
                    event,
                    Self::nick_char,
                    false,
                ),
            },
            State::Nick(ed) => match event {
                key!(Esc) => {
                    self.state = State::Normal;
                    true
                }
                key!(Enter) => {
                    if let Some(room) = &self.room {
                        let _ = room.nick(ed.text());
                    }
                    self.state = State::Normal;
                    true
                }
                _ => util::handle_editor_input_event(
                    ed,
                    terminal,
                    crossterm_lock,
                    event,
                    Self::nick_char,
                    false,
                ),
            },
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
        // These packets don't result in any noticeable change in the UI. This
        // function's main purpose is to prevent pings from causing a redraw.

        #[allow(clippy::match_like_matches_macro)]
        match data {
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
        }
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
