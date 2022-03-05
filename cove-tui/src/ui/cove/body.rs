use crossterm::event::{KeyCode, KeyEvent};
use tui::backend::Backend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::text::Span;
use tui::widgets::Paragraph;
use tui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::client::cove::conn::{State, Status};
use crate::client::cove::room::CoveRoom;
use crate::ui::input::EventHandler;
use crate::ui::textline::{TextLine, TextLineState};
use crate::ui::{layout, styles};

pub enum Body {
    Empty,
    Connecting,
    ChoosingRoom,
    Identifying,
    ChooseNick {
        nick: TextLineState,
        prev_error: Option<String>,
    },
    Present,
    Stopped, // TODO Display reason for stoppage
}

impl Default for Body {
    fn default() -> Self {
        Self::Empty
    }
}

impl Body {
    pub async fn update(&mut self, room: &CoveRoom) {
        match &*room.conn().await.state().await {
            State::Connecting => *self = Self::Connecting,
            State::Connected(conn) => match conn.status() {
                Status::ChoosingRoom => *self = Self::ChoosingRoom,
                Status::Identifying => *self = Self::Identifying,
                Status::IdRequired(error) => self.choose_nick(error.clone()),
                Status::Present(_) => *self = Self::Present,
            },
            State::Stopped => *self = Self::Stopped,
        }
    }

    fn choose_nick(&mut self, error: Option<String>) {
        match self {
            Self::ChooseNick { prev_error, .. } => *prev_error = error,
            _ => {
                *self = Self::ChooseNick {
                    nick: TextLineState::default(),
                    prev_error: error,
                }
            }
        }
    }

    pub async fn render<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        match self {
            Body::Empty => todo!(),
            Body::Connecting => {
                let text = "Connecting...";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Body::ChoosingRoom => {
                let text = "Entering room...";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Body::Identifying => {
                let text = "Identifying...";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Body::ChooseNick {
                nick,
                prev_error: None,
            } => {
                let area = layout::centered_v(2, area);
                let areas = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Length(1)])
                    .split(area);
                let title_area = areas[0];
                let text_area = areas[1];

                frame.render_widget(
                    Paragraph::new(Span::styled("Choose a nick:", styles::title()))
                        .alignment(Alignment::Center),
                    title_area,
                );
                frame.render_stateful_widget(TextLine, layout::centered(50, 1, text_area), nick);
            }
            Body::ChooseNick {
                nick,
                prev_error: Some(error),
            } => {
                let area = layout::centered_v(3, area);
                let areas = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(area);
                let title_area = areas[0];
                let text_area = areas[1];
                let error_area = areas[2];

                frame.render_widget(
                    Paragraph::new(Span::styled("Choose a nick:", styles::title()))
                        .alignment(Alignment::Center),
                    title_area,
                );
                frame.render_stateful_widget(TextLine, layout::centered(50, 1, text_area), nick);
                frame.render_widget(
                    Paragraph::new(Span::styled(error as &str, styles::error()))
                        .alignment(Alignment::Center),
                    error_area,
                );
            }
            Body::Present => {
                let text = "Present";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
            Body::Stopped => {
                let text = "Stopped";
                let area = layout::centered(text.width() as u16, 1, area);
                frame.render_widget(Paragraph::new(Span::styled(text, styles::title())), area);
            }
        }
    }
}

pub enum Reaction {
    Handled,
    Identify(String),
}

impl EventHandler for Body {
    type Reaction = Reaction;

    fn handle_key(&mut self, event: KeyEvent) -> Option<Self::Reaction> {
        match self {
            Body::ChooseNick { nick, .. } => {
                if event.code == KeyCode::Enter {
                    Some(Reaction::Identify(nick.content().to_string()))
                } else {
                    nick.handle_key(event).and(Some(Reaction::Handled))
                }
            }
            Body::Present => None, // TODO Implement
            _ => None,
        }
    }
}
