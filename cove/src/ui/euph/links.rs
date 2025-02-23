use cove_config::{Config, Keys};
use cove_input::InputEvent;
use crossterm::{event::KeyCode, style::Stylize};
use linkify::{LinkFinder, LinkKind};
use toss::{
    Style, Styled, Widget, WidgetExt,
    widgets::{Join2, Text},
};

use crate::{
    euph::{self, SpanType},
    ui::{
        UiError, key_bindings, util,
        widgets::{ListBuilder, ListState, Popup},
    },
};

use super::popup::PopupResult;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Link {
    Url(String),
    Room(String),
}

pub struct LinksState {
    config: &'static Config,
    links: Vec<Link>,
    list: ListState<usize>,
}

const NUMBER_KEYS: [char; 10] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

impl LinksState {
    pub fn new(config: &'static Config, content: &str) -> Self {
        let mut links = vec![];

        // Collect URL-like links
        for link in LinkFinder::new()
            .url_must_have_scheme(false)
            .kinds(&[LinkKind::Url])
            .links(content)
        {
            links.push((
                link.start(),
                link.end(),
                Link::Url(link.as_str().to_string()),
            ));
        }

        // Collect room links
        for (span, range) in euph::find_spans(content) {
            if span == SpanType::Room {
                let name = &content[range.start + 1..range.end];
                links.push((range.start, range.end, Link::Room(name.to_string())));
            }
        }

        links.sort();
        let links = links
            .into_iter()
            .map(|(_, _, link)| link)
            .collect::<Vec<_>>();

        Self {
            config,
            links,
            list: ListState::new(),
        }
    }

    pub fn widget(&mut self) -> impl Widget<UiError> {
        let style_selected = Style::new().black().on_white();

        let mut list_builder = ListBuilder::new();

        if self.links.is_empty() {
            list_builder.add_unsel(Text::new(("No links found", Style::new().grey().italic())))
        }

        for (id, link) in self.links.iter().enumerate() {
            let link = link.clone();
            list_builder.add_sel(id, move |selected| {
                let mut text = Styled::default();

                // Number key indicator
                text = match NUMBER_KEYS.get(id) {
                    None if selected => text.then("    ", style_selected),
                    None => text.then_plain("    "),
                    Some(key) if selected => text.then(format!("[{key}] "), style_selected.bold()),
                    Some(key) => text.then(format!("[{key}] "), Style::new().dark_grey().bold()),
                };

                // The link itself
                text = match link {
                    Link::Url(url) if selected => text.then(url, style_selected),
                    Link::Url(url) => text.then_plain(url),
                    Link::Room(name) if selected => {
                        text.then(format!("&{name}"), style_selected.bold())
                    }
                    Link::Room(name) => text.then(format!("&{name}"), Style::new().blue().bold()),
                };

                Text::new(text)
            });
        }

        let hint_style = Style::new().grey().italic();
        let hint = Styled::new("Open links with ", hint_style)
            .and_then(key_bindings::format_binding(
                &self.config.keys.general.confirm,
            ))
            .then(" or the number keys.", hint_style);

        Popup::new(
            Join2::vertical(
                list_builder.build(&mut self.list).segment(),
                Text::new(hint)
                    .padding()
                    .with_top(1)
                    .segment()
                    .with_fixed(true),
            ),
            "Links",
        )
    }

    fn open_link_by_id(&self, id: usize) -> PopupResult {
        match self.links.get(id) {
            Some(Link::Url(url)) => {
                // The `http://` or `https://` schema is necessary for
                // open::that to successfully open the link in the browser.
                let link = if url.starts_with("http://") || url.starts_with("https://") {
                    url.clone()
                } else {
                    format!("https://{url}")
                };

                if let Err(error) = open::that(&link) {
                    return PopupResult::ErrorOpeningLink { link, error };
                }
            }

            Some(Link::Room(name)) => return PopupResult::SwitchToRoom { name: name.clone() },

            _ => {}
        }
        PopupResult::Handled
    }

    fn open_link(&self) -> PopupResult {
        if let Some(id) = self.list.selected() {
            self.open_link_by_id(*id)
        } else {
            PopupResult::Handled
        }
    }

    pub fn handle_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> PopupResult {
        if event.matches(&keys.general.abort) {
            return PopupResult::Close;
        }

        if event.matches(&keys.general.confirm) {
            return self.open_link();
        }

        if util::handle_list_input_event(&mut self.list, event, keys) {
            return PopupResult::Handled;
        }

        if let Some(key_event) = event.key_event() {
            if key_event.modifiers.is_empty() {
                match key_event.code {
                    KeyCode::Char('1') => return self.open_link_by_id(0),
                    KeyCode::Char('2') => return self.open_link_by_id(1),
                    KeyCode::Char('3') => return self.open_link_by_id(2),
                    KeyCode::Char('4') => return self.open_link_by_id(3),
                    KeyCode::Char('5') => return self.open_link_by_id(4),
                    KeyCode::Char('6') => return self.open_link_by_id(5),
                    KeyCode::Char('7') => return self.open_link_by_id(6),
                    KeyCode::Char('8') => return self.open_link_by_id(7),
                    KeyCode::Char('9') => return self.open_link_by_id(8),
                    KeyCode::Char('0') => return self.open_link_by_id(9),
                    _ => {}
                }
            }
        }

        PopupResult::NotHandled
    }
}
