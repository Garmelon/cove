mod users;

use tui::backend::Backend;
use tui::layout::Rect;
use tui::Frame;

use crate::cove::room::CoveRoom;

pub struct CoveUi {
    room: CoveRoom,
}

impl CoveUi {
    pub fn new(room: CoveRoom) -> Self {
        Self { room }
    }

    pub async fn render_main<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Implement
    }

    pub async fn render_users<B: Backend>(&mut self, frame: &mut Frame<'_, B>, area: Rect) {
        // TODO Implement
    }
}
