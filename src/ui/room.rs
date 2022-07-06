use std::iter;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::{Color, ContentStyle, Stylize};
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::frame::{Frame, Pos, Size};
use toss::styled::Styled;
use toss::terminal::Terminal;

use crate::euph::api::{SessionType, SessionView};
use crate::euph::{self, Joined, Status};
use crate::vault::{EuphMsg, EuphVault};

use super::chat::Chat;
use super::list::{List, Row};
use super::{util, UiEvent};

pub struct EuphRoom {
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    room: Option<euph::Room>,
    chat: Chat<EuphMsg, EuphVault>,

    nick_list_width: u16,
    nick_list: List<String>,
}

impl EuphRoom {
    pub fn new(vault: EuphVault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            ui_event_tx,
            room: None,
            chat: Chat::new(vault),
            nick_list_width: 24,
            nick_list: List::new(),
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

    pub async fn render(&mut self, frame: &mut Frame) {
        let status = self.status().await;
        match &status {
            Some(Some(Status::Joined(joined))) => {
                self.render_with_nick_list(frame, &status, joined).await
            }
            _ => self.render_without_nick_list(frame, &status).await,
        }
    }

    async fn render_without_nick_list(
        &mut self,
        frame: &mut Frame,
        status: &Option<Option<Status>>,
    ) {
        let size = frame.size();

        // Position of horizontal line between status and chat
        let hsplit = 1_i32;

        let status_pos = Pos::new(0, 0);
        // let status_size = Size::new(size.width, 1);

        let chat_pos = Pos::new(0, hsplit + 1);
        let chat_size = Size::new(size.width, size.height.saturating_sub(hsplit as u16 + 1));

        self.chat.render(frame, chat_pos, chat_size).await;
        self.render_status(frame, status_pos, status);
        Self::render_hsplit(frame, hsplit);
    }

    async fn render_with_nick_list(
        &mut self,
        frame: &mut Frame,
        status: &Option<Option<Status>>,
        joined: &Joined,
    ) {
        let size = frame.size();

        // Position of vertical line between main part and nick list
        let vsplit = size.width.saturating_sub(self.nick_list_width + 1) as i32;
        // Position of horizontal line between status and chat
        let hsplit = 1_i32;

        let status_pos = Pos::new(0, 0);
        // let status_size = Size::new(vsplit as u16, 1);

        let chat_pos = Pos::new(0, hsplit + 1);
        let chat_size = Size::new(vsplit as u16, size.height.saturating_sub(hsplit as u16 + 1));

        let nick_list_pos = Pos::new(vsplit + 1, 0);
        let nick_list_size = Size::new(self.nick_list_width, size.height);

        self.chat.render(frame, chat_pos, chat_size).await;
        self.render_status(frame, status_pos, status);
        self.render_nick_list(frame, nick_list_pos, nick_list_size, joined);
        Self::render_vsplit_hsplit(frame, vsplit, hsplit);
    }

    fn render_status(&self, frame: &mut Frame, pos: Pos, status: &Option<Option<Status>>) {
        let room = self.chat.store().room();
        let room_style = ContentStyle::default().bold().blue();
        let mut info = Styled::new((format!("&{room}"), room_style));
        info = match status {
            None => info.then(", archive"),
            Some(None) => info.then(", connecting..."),
            Some(Some(Status::Joining(j))) if j.bounce.is_some() => info.then(", auth required"),
            Some(Some(Status::Joining(_))) => info.then(", joining..."),
            Some(Some(Status::Joined(j))) => {
                let nick = &j.session.name;
                if nick.is_empty() {
                    info.then(", present without nick")
                } else {
                    let nick_style = euph::nick_style(nick);
                    info.then(", present as ").then((nick, nick_style))
                }
            }
        };
        frame.write(pos, info);
    }

    fn render_row(session: &SessionView) -> Row<String> {
        if session.name.is_empty() {
            let name = "lurk";
            let style = ContentStyle::default().grey();
            let style_inv = ContentStyle::default().black().on_grey();
            Row::sel(
                session.session_id.clone(),
                Styled::new((name, style)),
                style,
                Styled::new((name, style_inv)),
                style_inv,
            )
        } else {
            let name = &session.name;
            let (r, g, b) = euph::nick_color(name);
            let color = Color::Rgb { r, g, b };
            let style = ContentStyle::default().bold().with(color);
            let style_inv = ContentStyle::default().bold().black().on(color);
            Row::sel(
                session.session_id.clone(),
                Styled::new((name, style)),
                style,
                Styled::new((name, style_inv)),
                style_inv,
            )
        }
    }

    fn render_section(rows: &mut Vec<Row<String>>, name: &str, sessions: &[&SessionView]) {
        if sessions.is_empty() {
            return;
        }

        let heading_style = ContentStyle::new().bold();

        if !rows.is_empty() {
            rows.push(Row::unsel(""));
        }

        let row = Styled::new((name, heading_style)).then(format!(" ({})", sessions.len()));
        rows.push(Row::unsel(row));

        for sess in sessions {
            rows.push(Self::render_row(sess));
        }
    }

    fn render_rows(joined: &Joined) -> Vec<Row<String>> {
        let mut people = vec![];
        let mut bots = vec![];
        let mut lurkers = vec![];
        let mut nurkers = vec![];

        for sess in iter::once(&joined.session).chain(joined.listing.values()) {
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

        let mut rows: Vec<Row<String>> = vec![];
        Self::render_section(&mut rows, "People", &people);
        Self::render_section(&mut rows, "Bots", &bots);
        Self::render_section(&mut rows, "Lurkers", &lurkers);
        Self::render_section(&mut rows, "Nurkers", &nurkers);
        rows
    }

    fn render_nick_list(&mut self, frame: &mut Frame, pos: Pos, size: Size, joined: &Joined) {
        // Clear area in case there's overdraw from the chat or status
        for y in pos.y..(pos.y + size.height as i32) {
            for x in pos.x..(pos.x + size.width as i32) {
                frame.write(Pos::new(x, y), " ");
            }
        }

        let rows = Self::render_rows(joined);
        self.nick_list.render(frame, pos, size, rows);
    }

    fn render_hsplit(frame: &mut Frame, hsplit: i32) {
        for x in 0..frame.size().width as i32 {
            frame.write(Pos::new(x, hsplit), "─");
        }
    }

    fn render_vsplit_hsplit(frame: &mut Frame, vsplit: i32, hsplit: i32) {
        for x in 0..vsplit {
            frame.write(Pos::new(x, hsplit), "─");
        }

        for y in 0..frame.size().height as i32 {
            let symbol = if y == hsplit { "┤" } else { "│" };
            frame.write(Pos::new(vsplit, y), symbol);
        }
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        size: Size,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) {
        let chat_size = Size {
            height: size.height - 2,
            ..size
        };
        self.chat
            .handle_navigation(terminal, chat_size, event)
            .await;

        if let Some(room) = &self.room {
            if let Ok(Some(Status::Joined(_))) = room.status().await {
                if let KeyCode::Char('n' | 'N') = event.code {
                    if let Some(new_nick) = util::prompt(terminal, crossterm_lock) {
                        let _ = room.nick(new_nick);
                    }
                }

                if let Some((parent, content)) = self
                    .chat
                    .handle_messaging(terminal, crossterm_lock, event)
                    .await
                {
                    let _ = room.send(parent, content);
                }
            }
        }
    }
}
