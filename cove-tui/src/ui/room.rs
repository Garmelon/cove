mod users;

use std::sync::Arc;

use tokio::sync::Mutex;
use tui::backend::Backend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::Style;
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, BorderType, Borders, Paragraph};
use tui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::room::{Room, Status};

use self::users::Users;

use super::textline::{TextLine, TextLineState};
use super::{layout, styles};

enum Main {
    Empty,
    Connecting,
    Identifying,
    ChooseNick {
        nick: TextLineState,
        prev_error: Option<String>,
    },
    Messages,
    FatalError(String),
}

impl Main {
    fn choose_nick() -> Self {
        Self::ChooseNick {
            nick: TextLineState::default(),
            prev_error: None,
        }
    }

    fn fatal<S: ToString>(s: S) -> Self {
        Self::FatalError(s.to_string())
    }
}

pub struct RoomInfo {
    name: String,
    room: Arc<Mutex<Room>>,
    main: Main,
}

impl RoomInfo {
    pub fn new(name: String, room: Arc<Mutex<Room>>) -> Self {
        Self {
            name,
            room,
            main: Main::Empty,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    async fn align_main(&mut self) {
        let room = self.room.lock().await;
        match room.status() {
            Status::Nominal if room.connected() && room.present().is_some() => {
                if !matches!(self.main, Main::Messages) {
                    self.main = Main::Messages;
                }
            }
            Status::Nominal if room.connected() => self.main = Main::Connecting,
            Status::Nominal => self.main = Main::Identifying,
            Status::NickRequired => self.main = Main::choose_nick(),
            Status::CouldNotConnect => self.main = Main::fatal("Could not connect to room"),
            Status::InvalidRoom(err) => self.main = Main::fatal(format!("Invalid room:\n{err}")),
            Status::InvalidNick(err) => {
                if let Main::ChooseNick { prev_error, .. } = &mut self.main {
                    *prev_error = Some(err.clone());
                } else {
                    self.main = Main::choose_nick();
                }
            }
            Status::InvalidIdentity(err) => {
                self.main = Main::fatal(format!("Invalid identity:\n{err}"))
            }
        }
    }

    pub async fn render_main<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        self.align_main().await;

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(area);
        let room_name_area = areas[0];
        let separator_area = areas[1];
        let main_area = areas[2];

        // Room name at the top
        let room_name = Paragraph::new(Span::styled(
            format!("&{}", self.name()),
            styles::selected_room(),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(room_name, room_name_area);
        let separator = Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Double);
        frame.render_widget(separator, separator_area);

        // Main below
        self.render_main_inner(frame, main_area).await;
    }

    async fn render_main_inner<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        match &mut self.main {
            Main::Empty => {}
            Main::Connecting => {
                let text = "Connecing...";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Main::Identifying => {
                let text = "Identifying...";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Main::ChooseNick {
                nick,
                prev_error: None,
            } => {
                let area = layout::centered(50, 2, area);
                let top = Rect { height: 1, ..area };
                let bot = Rect {
                    y: top.y + 1,
                    ..top
                };
                let text = "Choose a nick:";
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), top);
                frame.render_stateful_widget(TextLine, bot, nick);
            }
            Main::ChooseNick { nick, prev_error } => {
                let width = prev_error
                    .as_ref()
                    .map(|e| e.width() as u16)
                    .unwrap_or(0)
                    .max(50);
                let height = if prev_error.is_some() { 5 } else { 2 };
                let area = layout::centered(width, height, area);
                let top = Rect {
                    height: height - 1,
                    ..area
                };
                let bot = Rect {
                    y: area.bottom() - 1,
                    height: 1,
                    ..area
                };
                let mut lines = vec![];
                if let Some(err) = &prev_error {
                    lines.push(Spans::from(Span::styled("Error:", styles::title())));
                    lines.push(Spans::from(Span::styled(err, styles::error())));
                    lines.push(Spans::from(""));
                }
                lines.push(Spans::from(Span::styled("Choose a nick:", styles::title())));
                frame.render_widget(Paragraph::new(lines), top);
                frame.render_stateful_widget(TextLine, bot, nick);
            }
            Main::Messages => {
                // TODO Actually render messages
                frame.render_widget(Paragraph::new("TODO: Messages"), area);
            }
            Main::FatalError(err) => {
                let title = "Fatal error:";
                let width = (err.width() as u16).max(title.width() as u16);
                let area = layout::centered(width, 2, area);
                let pg = Paragraph::new(vec![
                    Spans::from(Span::styled(title, styles::title())),
                    Spans::from(Span::styled(err as &str, styles::error())),
                ])
                .alignment(Alignment::Center);
                frame.render_widget(pg, area);
            }
        }
    }

    pub async fn render_users<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        if let Some(present) = self.room.lock().await.present() {
            frame.render_widget(Users::new(present), area);
        }
    }
}
