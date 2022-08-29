use std::io;
use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use parking_lot::FairMutex;
use toss::terminal::Terminal;

use crate::euph::Room;
use crate::ui::input::{key, InputEvent, KeyBindingsList, KeyEvent};
use crate::ui::widgets::list::ListState;
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

pub struct LinksState {
    links: Vec<String>,
    list: ListState<usize>,
}

pub enum EventResult {
    NotHandled,
    Handled,
    Close,
    ErrorOpeningLink { link: String, error: io::Error },
}

impl LinksState {
    pub fn new(content: &str) -> Self {
        // TODO Extract links
        Self {
            links: vec![
                "https://example.com/".to_string(),
                "https://plugh.de/".to_string(),
            ],
            list: ListState::new(),
        }
    }

    pub fn widget(&self) -> BoxedWidget {
        let mut list = self.list.widget();
        for (id, link) in self.links.iter().enumerate() {
            list.add_sel(
                id,
                Text::new((link,)),
                Text::new((link, ContentStyle::default().black().on_white())),
            );
        }

        Popup::new(list).title("Links").build()
    }

    pub fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close links popup")
    }

    pub fn handle_input_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: &InputEvent,
        room: &Option<Room>,
    ) -> EventResult {
        match event {
            key!(Esc) => EventResult::Close,
            _ => EventResult::NotHandled,
        }
    }
}
