mod body;
mod users;

use crossterm::event::KeyEvent;
use tui::backend::Backend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::text::Span;
use tui::widgets::{Block, BorderType, Borders, Paragraph};
use tui::Frame;

use crate::client::cove::room::CoveRoom;

use self::body::{Body, Reaction};
use self::users::CoveUsers;

use super::input::EventHandler;
use super::styles;

pub struct CoveUi {
    room: CoveRoom,
    body: Body,
}

impl CoveUi {
    pub fn new(room: CoveRoom) -> Self {
        Self {
            room,
            body: Body::default(),
        }
    }

    fn name(&self) -> &str {
        self.room.name()
    }

    pub async fn render_main<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(area);
        let title_area = areas[0];
        let separator_area = areas[1];
        let body_area = areas[2];

        self.render_title(frame, title_area).await;
        self.render_separator(frame, separator_area).await;
        self.render_body(frame, body_area).await;
    }

    async fn render_title<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Show current nick as well, if applicable
        let room_name = Paragraph::new(Span::styled(
            format!("&{}", self.name()),
            styles::selected_room(),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(room_name, area);
    }

    async fn render_separator<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        let separator = Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Double);
        frame.render_widget(separator, area);
    }

    async fn render_body<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        self.body.update(&self.room).await;
        self.body.render(frame, area).await
    }

    pub async fn render_users<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        if let Some(present) = self.room.conn().await.state().await.present() {
            frame.render_widget(CoveUsers::new(present), area);
        }
    }

    pub async fn handle_key(&mut self, event: KeyEvent) -> Option<()> {
        match self.body.handle_key(event)? {
            Reaction::Handled => Some(()),
            Reaction::Identify(nick) => {
                self.room.identify(&nick, &nick).await;
                Some(())
            }
        }
    }
}
