use cove_config::{Config, Keys};
use cove_input::InputEvent;
use crossterm::event::KeyCode;
use crossterm::style::Stylize;
use linkify::{LinkFinder, LinkKind};
use toss::widgets::{Join2, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::ui::widgets::{ListBuilder, ListState, Popup};
use crate::ui::{UiError, key_bindings, util};

use super::popup::PopupResult;

pub struct LinksState {
    config: &'static Config,
    links: Vec<String>,
    list: ListState<usize>,
}

const NUMBER_KEYS: [char; 10] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

impl LinksState {
    pub fn new(config: &'static Config, content: &str) -> Self {
        let links = LinkFinder::new()
            .url_must_have_scheme(false)
            .kinds(&[LinkKind::Url])
            .links(content)
            .map(|l| l.as_str().to_string())
            .collect();

        Self {
            config,
            links,
            list: ListState::new(),
        }
    }

    pub fn widget(&mut self) -> impl Widget<UiError> + '_ {
        let style_selected = Style::new().black().on_white();

        let mut list_builder = ListBuilder::new();

        if self.links.is_empty() {
            list_builder.add_unsel(Text::new(("No links found", Style::new().grey().italic())))
        }

        for (id, link) in self.links.iter().enumerate() {
            let link = link.clone();
            if let Some(&number_key) = NUMBER_KEYS.get(id) {
                list_builder.add_sel(id, move |selected| {
                    let text = if selected {
                        Styled::new(format!("[{number_key}]"), style_selected.bold())
                            .then(" ", style_selected)
                            .then(link, style_selected)
                    } else {
                        Styled::new(format!("[{number_key}]"), Style::new().dark_grey().bold())
                            .then_plain(" ")
                            .then_plain(link)
                    };
                    Text::new(text)
                });
            } else {
                list_builder.add_sel(id, move |selected| {
                    let text = if selected {
                        Styled::new(format!("    {link}"), style_selected)
                    } else {
                        Styled::new_plain(format!("    {link}"))
                    };
                    Text::new(text)
                });
            }
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
        if let Some(link) = self.links.get(id) {
            // The `http://` or `https://` schema is necessary for open::that to
            // successfully open the link in the browser.
            let link = if link.starts_with("http://") || link.starts_with("https://") {
                link.clone()
            } else {
                format!("https://{link}")
            };

            if let Err(error) = open::that(&link) {
                return PopupResult::ErrorOpeningLink { link, error };
            }
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
