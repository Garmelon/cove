use std::io;

use crossterm::event::KeyCode;
use crossterm::style::{ContentStyle, Stylize};
use linkify::{LinkFinder, LinkKind};

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
        let links = LinkFinder::new()
            .links(content)
            .filter(|l| *l.kind() == LinkKind::Url)
            .map(|l| l.as_str().to_string())
            .collect();

        Self {
            links,
            list: ListState::new(),
        }
    }

    pub fn widget(&self) -> BoxedWidget {
        let mut list = self.list.widget().focus(true);
        for (id, link) in self.links.iter().enumerate() {
            list.add_sel(
                id,
                Text::new((link,)),
                Text::new((link, ContentStyle::default().black().on_white())),
            );
        }

        Popup::new(list).title("Links").build()
    }

    pub fn open_link(&self) -> EventResult {
        if let Some(id) = self.list.cursor() {
            if let Some(link) = self.links.get(id) {
                if let Err(error) = open::that(link) {
                    return EventResult::ErrorOpeningLink {
                        link: link.to_string(),
                        error,
                    };
                }
            }
        }
        EventResult::Handled
    }

    pub fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close links popup");
        bindings.binding("j/k, ↓/↑", "move cursor up/down");
        bindings.binding("g, home", "move cursor to top");
        bindings.binding("G, end", "move cursor to bottom");
        bindings.binding("ctrl+y/e", "scroll up/down");
        bindings.empty();
        bindings.binding("enter", "open selected link");
    }

    pub fn handle_input_event(&mut self, event: &InputEvent) -> EventResult {
        match event {
            key!(Esc) => return EventResult::Close,
            key!('k') | key!(Up) => self.list.move_cursor_up(),
            key!('j') | key!(Down) => self.list.move_cursor_down(),
            key!('g') | key!(Home) => self.list.move_cursor_to_top(),
            key!('G') | key!(End) => self.list.move_cursor_to_bottom(),
            key!(Ctrl + 'y') => self.list.scroll_up(1),
            key!(Ctrl + 'e') => self.list.scroll_down(1),
            key!(Enter) => return self.open_link(),
            _ => return EventResult::NotHandled,
        }
        EventResult::Handled
    }
}
