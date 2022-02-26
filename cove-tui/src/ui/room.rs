mod users;

use std::sync::Arc;

use tokio::sync::Mutex;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::widgets::{Block, BorderType, Borders};
use tui::Frame;

use crate::room::Room;

use self::users::Users;

pub struct RoomInfo {
    name: String,
    room: Arc<Mutex<Room>>,
}

impl RoomInfo {
    pub fn new(name: String, room: Arc<Mutex<Room>>) -> Self {
        Self { name, room }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn render_main<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Implement
        frame.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Double),
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: 1,
            },
        );
    }

    pub async fn render_users<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        if let Some(present) = self.room.lock().await.present() {
            frame.render_widget(Users::new(present), area);
        }
    }
}
