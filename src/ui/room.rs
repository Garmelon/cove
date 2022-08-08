use std::iter;
use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{Color, ContentStyle, Stylize};
use parking_lot::FairMutex;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{mpsc, oneshot};
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::api::{SessionType, SessionView, Snowflake};
use crate::euph::{self, Joined, Status};
use crate::store::MsgStore;
use crate::vault::EuphVault;

use super::chat::{ChatState, Reaction};
use super::input::{key, KeyBindingsList, KeyEvent};
use super::widgets::background::Background;
use super::widgets::border::Border;
use super::widgets::editor::EditorState;
use super::widgets::empty::Empty;
use super::widgets::float::Float;
use super::widgets::join::{HJoin, Segment, VJoin};
use super::widgets::layer::Layer;
use super::widgets::list::{List, ListState};
use super::widgets::padding::Padding;
use super::widgets::text::Text;
use super::widgets::BoxedWidget;
use super::{util, UiEvent};

enum State {
    Normal,
    ChooseNick(EditorState),
}

pub struct EuphRoom {
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    vault: EuphVault,
    room: Option<euph::Room>,

    state: State,

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
            chat: ChatState::new(vault),
            last_msg_sent: None,
            nick_list: ListState::new(),
        }
    }

    pub fn connect(&mut self) {
        if self.room.is_none() {
            self.room = Some(euph::Room::new(
                self.chat.store().clone(),
                self.ui_event_tx.clone(),
            ));
        }
    }

    pub fn disconnect(&mut self) {
        self.room = None;
    }

    pub async fn status(&self) -> Option<Option<Status>> {
        if let Some(room) = &self.room {
            room.status().await.ok()
        } else {
            None
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

    pub async fn widget(&mut self) -> BoxedWidget {
        self.stabilize_pseudo_msg().await;

        let status = self.status().await;
        let chat = match &status {
            Some(Some(Status::Joined(joined))) => self.widget_with_nick_list(&status, joined),
            _ => self.widget_without_nick_list(&status),
        };
        match &self.state {
            State::Normal => chat,
            State::ChooseNick(ed) => Layer::new(vec![
                chat,
                Float::new(Border::new(Background::new(VJoin::new(vec![
                    Segment::new(Padding::new(Text::new("Choose nick")).horizontal(1)),
                    Segment::new(
                        Padding::new(
                            ed.widget()
                                .highlight(|s| Styled::new(s, euph::nick_style(s))),
                        )
                        .left(1),
                    ),
                ]))))
                .horizontal(0.5)
                .vertical(0.5)
                .into(),
            ])
            .into(),
        }
    }

    fn widget_without_nick_list(&self, status: &Option<Option<Status>>) -> BoxedWidget {
        VJoin::new(vec![
            Segment::new(Border::new(
                Padding::new(self.status_widget(status)).horizontal(1),
            )),
            // TODO Use last known nick?
            Segment::new(self.chat.widget(String::new())).expanding(true),
        ])
        .into()
    }

    fn widget_with_nick_list(
        &self,
        status: &Option<Option<Status>>,
        joined: &Joined,
    ) -> BoxedWidget {
        HJoin::new(vec![
            Segment::new(VJoin::new(vec![
                Segment::new(Border::new(
                    Padding::new(self.status_widget(status)).horizontal(1),
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

    fn status_widget(&self, status: &Option<Option<Status>>) -> BoxedWidget {
        // TODO Include unread message count
        let room = self.chat.store().room();
        let room_style = ContentStyle::default().bold().blue();
        let mut info = Styled::new(format!("&{room}"), room_style);
        info = match status {
            None => info.then_plain(", archive"),
            Some(None) => info.then_plain(", connecting..."),
            Some(Some(Status::Joining(j))) if j.bounce.is_some() => {
                info.then_plain(", auth required")
            }
            Some(Some(Status::Joining(_))) => info.then_plain(", joining..."),
            Some(Some(Status::Joined(j))) => {
                let nick = &j.session.name;
                if nick.is_empty() {
                    info.then_plain(", present without nick")
                } else {
                    let nick_style = euph::nick_style(nick);
                    info.then_plain(", present as ").then(nick, nick_style)
                }
            }
        };
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

        match &self.state {
            State::Normal => {
                // TODO Use if-let chain
                bindings.binding("esc", "leave room");
                let can_compose = if let Some(room) = &self.room {
                    if let Ok(Some(Status::Joined(_))) = room.status().await {
                        bindings.binding("n", "change nick");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                bindings.empty();
                self.chat.list_key_bindings(bindings, can_compose).await;
            }
            State::ChooseNick(_) => {
                bindings.binding("esc", "abort");
                bindings.binding("enter", "set nick");
                util::list_editor_key_bindings(bindings, Self::nick_char, false);
            }
        }
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> bool {
        match &self.state {
            State::Normal => {
                // TODO Use if-let chain
                if let Some(room) = &self.room {
                    if let Ok(Some(Status::Joined(joined))) = room.status().await {
                        match self
                            .chat
                            .handle_key_event(terminal, crossterm_lock, event, true)
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

                        if let key!('n') | key!('N') = event {
                            self.state = State::ChooseNick(EditorState::with_initial_text(
                                joined.session.name.clone(),
                            ));
                            return true;
                        }

                        return false;
                    }
                }

                self.chat
                    .handle_key_event(terminal, crossterm_lock, event, false)
                    .await
                    .handled()
            }
            State::ChooseNick(ed) => match event {
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
                _ => util::handle_editor_key_event(
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
}
