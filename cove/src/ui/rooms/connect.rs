use cove_config::Keys;
use cove_input::InputEvent;
use crossterm::style::Stylize;
use toss::{
    Style, Styled, Widget, WidgetExt,
    widgets::{EditorState, Empty, Join2, Join3, Text},
};

use crate::{
    ui::{UiError, util, widgets::Popup},
    vault::RoomIdentifier,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Name,
    Domain,
}

impl Focus {
    fn advance(self) -> Self {
        match self {
            Self::Name => Self::Domain,
            Self::Domain => Self::Name,
        }
    }
}

pub struct ConnectState {
    focus: Focus,
    name: EditorState,
    domain: EditorState,
}

pub enum ConnectResult {
    Close,
    Connect(RoomIdentifier),
    Handled,
    Unhandled,
}

impl ConnectState {
    pub fn new() -> Self {
        Self {
            focus: Focus::Name,
            name: EditorState::new(),
            domain: EditorState::with_initial_text("euphoria.leet.nu".to_string()),
        }
    }

    pub fn handle_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> ConnectResult {
        if event.matches(&keys.general.abort) {
            return ConnectResult::Close;
        }

        if event.matches(&keys.general.focus) {
            self.focus = self.focus.advance();
            return ConnectResult::Handled;
        }

        if event.matches(&keys.general.confirm) {
            let id = RoomIdentifier {
                domain: self.domain.text().to_string(),
                name: self.name.text().to_string(),
            };
            if !id.domain.is_empty() && !id.name.is_empty() {
                return ConnectResult::Connect(id);
            }
        }

        let handled = match self.focus {
            Focus::Name => {
                util::handle_editor_input_event(&mut self.name, event, keys, util::is_room_char)
            }
            Focus::Domain => {
                util::handle_editor_input_event(&mut self.domain, event, keys, |c| c != '\n')
            }
        };

        if handled {
            return ConnectResult::Handled;
        }

        ConnectResult::Unhandled
    }

    pub fn widget(&mut self) -> impl Widget<UiError> + '_ {
        let room_style = Style::new().bold().blue();
        let domain_style = Style::new().grey();

        let name = Join2::horizontal(
            Text::new(Styled::new_plain("Room:   ").then("&", room_style))
                .with_wrap(false)
                .segment()
                .with_fixed(true),
            self.name
                .widget()
                .with_highlight(|s| Styled::new(s, room_style))
                .with_focus(self.focus == Focus::Name)
                .segment(),
        );

        let domain = Join3::horizontal(
            Text::new("Domain:")
                .with_wrap(false)
                .segment()
                .with_fixed(true),
            Empty::new().with_width(1).segment().with_fixed(true),
            self.domain
                .widget()
                .with_highlight(|s| Styled::new(s, domain_style))
                .with_focus(self.focus == Focus::Domain)
                .segment(),
        );

        let inner = Join2::vertical(
            name.segment().with_fixed(true),
            domain.segment().with_fixed(true),
        );

        Popup::new(inner, "Connect to")
    }
}
