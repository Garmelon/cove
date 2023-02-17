use std::io;

use crossterm::style::Stylize;
use linkify::{LinkFinder, LinkKind};
use toss::{Style, Styled};

use crate::ui::input::{key, InputEvent, KeyBindingsList};
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

const NUMBER_KEYS: [char; 10] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

impl LinksState {
    pub fn new(content: &str) -> Self {
        let links = LinkFinder::new()
            .url_must_have_scheme(false)
            .kinds(&[LinkKind::Url])
            .links(content)
            .map(|l| l.as_str().to_string())
            .collect();

        Self {
            links,
            list: ListState::new(),
        }
    }

    pub fn widget(&self) -> BoxedWidget {
        let style_selected = Style::new().black().on_white();

        let mut list = self.list.widget().focus(true);
        if self.links.is_empty() {
            list.add_unsel(Text::new(("No links found", Style::new().grey().italic())))
        }
        for (id, link) in self.links.iter().enumerate() {
            let (line_normal, line_selected) = if let Some(number_key) = NUMBER_KEYS.get(id) {
                (
                    Styled::new(format!("[{number_key}]"), Style::new().dark_grey().bold())
                        .then_plain(" ")
                        .then_plain(link),
                    Styled::new(format!("[{number_key}]"), style_selected.bold())
                        .then(" ", style_selected)
                        .then(link, style_selected),
                )
            } else {
                (
                    Styled::new_plain(format!("    {link}")),
                    Styled::new(format!("    {link}"), style_selected),
                )
            };

            list.add_sel(id, Text::new(line_normal), Text::new(line_selected));
        }

        Popup::new(list).title("Links").build()
    }

    fn open_link_by_id(&self, id: usize) -> EventResult {
        if let Some(link) = self.links.get(id) {
            // The `http://` or `https://` schema is necessary for open::that to
            // successfully open the link in the browser.
            let link = if link.starts_with("http://") || link.starts_with("https://") {
                link.clone()
            } else {
                format!("https://{link}")
            };

            if let Err(error) = open::that(&link) {
                return EventResult::ErrorOpeningLink { link, error };
            }
        }
        EventResult::Handled
    }

    fn open_link(&self) -> EventResult {
        if let Some(id) = self.list.cursor() {
            self.open_link_by_id(id)
        } else {
            EventResult::Handled
        }
    }

    pub fn list_key_bindings(&self, bindings: &mut KeyBindingsList) {
        bindings.binding("esc", "close links popup");
        bindings.binding("j/k, ↓/↑", "move cursor up/down");
        bindings.binding("g, home", "move cursor to top");
        bindings.binding("G, end", "move cursor to bottom");
        bindings.binding("ctrl+y/e", "scroll up/down");
        bindings.empty();
        bindings.binding("enter", "open selected link");
        bindings.binding("1,2,...", "open link by position");
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
            key!('1') => return self.open_link_by_id(0),
            key!('2') => return self.open_link_by_id(1),
            key!('3') => return self.open_link_by_id(2),
            key!('4') => return self.open_link_by_id(3),
            key!('5') => return self.open_link_by_id(4),
            key!('6') => return self.open_link_by_id(5),
            key!('7') => return self.open_link_by_id(6),
            key!('8') => return self.open_link_by_id(7),
            key!('9') => return self.open_link_by_id(8),
            key!('0') => return self.open_link_by_id(9),
            _ => return EventResult::NotHandled,
        }
        EventResult::Handled
    }
}
