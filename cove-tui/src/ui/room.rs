use std::sync::Arc;

use tokio::sync::Mutex;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::Frame;

use crate::room::Room;

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

    pub async fn render_messages<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Implement
    }

    pub async fn render_users<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Implement
    }
}
