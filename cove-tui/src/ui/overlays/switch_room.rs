use crossterm::event::{KeyCode, KeyEvent};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Block, Borders, Clear, StatefulWidget, Widget};

use crate::ui::input::EventHandler;
use crate::ui::layout;
use crate::ui::textline::{TextLine, TextLineReaction, TextLineState};

use super::OverlayReaction;

pub struct SwitchRoom;

impl StatefulWidget for SwitchRoom {
    type State = SwitchRoomState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = layout::centered(50, 3, area);
        Clear.render(area, buf);

        let block = Block::default().title("Join room").borders(Borders::ALL);
        let inner_area = block.inner(area);
        block.render(area, buf);

        TextLine.render(inner_area, buf, &mut state.room);
    }
}

#[derive(Debug, Default)]
pub struct SwitchRoomState {
    room: TextLineState,
}

impl EventHandler for SwitchRoomState {
    type Reaction = OverlayReaction;

    fn handle_key(&mut self, event: KeyEvent) -> Option<Self::Reaction> {
        if event.code == KeyCode::Enter {
            return Some(Self::Reaction::SwitchRoom(self.room.content()));
        }

        self.room.handle_key(event).map(|r| match r {
            TextLineReaction::Handled => Self::Reaction::Handled,
            TextLineReaction::Close => Self::Reaction::Close,
        })
    }
}

impl SwitchRoomState {
    pub fn last_cursor_pos(&self) -> (u16, u16) {
        self.room.last_cursor_pos()
    }
}
