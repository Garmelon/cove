use std::iter;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{Color, ContentStyle, Stylize};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::api::{SessionType, SessionView};
use crate::euph::{self, Joined, Status};
use crate::vault::{EuphMsg, EuphVault};

use super::chat::ChatState;
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
use super::UiEvent;

enum State {
    Normal,
    ChooseNick(EditorState),
}

pub struct EuphRoom {
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,

    state: State,

    room: Option<euph::Room>,
    chat: ChatState<EuphMsg, EuphVault>,
    nick_list: ListState<String>,
}

impl EuphRoom {
    pub fn new(vault: EuphVault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            ui_event_tx,
            state: State::Normal,
            room: None,
            chat: ChatState::new(vault),
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

    pub async fn widget(&self) -> BoxedWidget {
        let status = self.status().await;
        let chat = match &status {
            Some(Some(Status::Joined(joined))) => self.widget_with_nick_list(&status, joined),
            _ => self.widget_without_nick_list(&status),
        };
        match &self.state {
            State::Normal => chat,
            State::ChooseNick(ed) => Layer::new(vec![
                chat,
                Float::new(Border::new(Background::new(
                    Padding::new(VJoin::new(vec![
                        Segment::new(Text::new("Choose nick ")),
                        Segment::new(
                            ed.widget()
                                .highlight(|s| Styled::new(s, euph::nick_style(s))),
                        ),
                    ]))
                    .left(1),
                )))
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
            Segment::new(self.chat.widget()).expanding(true),
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
                Segment::new(self.chat.widget()).expanding(true),
            ]))
            .expanding(true),
            Segment::new(Border::new(
                Padding::new(self.nick_list_widget(joined)).horizontal(1),
            )),
        ])
        .into()
    }

    fn status_widget(&self, status: &Option<Option<Status>>) -> BoxedWidget {
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
            ""
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
            list.add_unsel(Empty);
        }

        let row = Styled::new(name, heading_style).then_plain(format!(" ({})", sessions.len()));
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

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) -> bool {
        match &self.state {
            State::Normal => {
                if self.chat.handle_navigation(event).await {
                    return true;
                }

                if let Some(room) = &self.room {
                    if let Ok(Some(Status::Joined(joined))) = room.status().await {
                        if let KeyCode::Char('n' | 'N') = event.code {
                            self.state = State::ChooseNick(EditorState::with_initial_text(
                                joined.session.name.clone(),
                            ));
                            return true;
                        }

                        let potential_message = self
                            .chat
                            .handle_messaging(terminal, crossterm_lock, event)
                            .await;
                        if let Some((parent, content)) = potential_message {
                            let _ = room.send(parent, content);
                            return true;
                        }
                    }
                }

                false
            }
            State::ChooseNick(ed) => {
                match event.code {
                    KeyCode::Esc => self.state = State::Normal,
                    KeyCode::Enter => {
                        if let Some(room) = &self.room {
                            let _ = room.nick(ed.text());
                        }
                        self.state = State::Normal;
                    }
                    KeyCode::Backspace => ed.backspace(),
                    KeyCode::Left => ed.move_cursor_left(),
                    KeyCode::Right => ed.move_cursor_right(),
                    KeyCode::Delete => ed.delete(),
                    KeyCode::Char(ch) => ed.insert_char(ch),
                    _ => return false,
                }
                true
            }
        }
    }
}
