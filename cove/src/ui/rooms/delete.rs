use cove_config::Keys;
use cove_input::InputEvent;
use crossterm::style::Stylize;
use toss::widgets::{EditorState, Empty, Join2, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::ui::widgets::Popup;
use crate::ui::{UiError, util};
use crate::vault::RoomIdentifier;

pub struct DeleteState {
    id: RoomIdentifier,
    name: EditorState,
}

pub enum DeleteResult {
    Close,
    Delete(RoomIdentifier),
    Handled,
    Unhandled,
}

impl DeleteState {
    pub fn new(id: RoomIdentifier) -> Self {
        Self {
            id,
            name: EditorState::new(),
        }
    }

    pub fn handle_input_event(&mut self, event: &mut InputEvent<'_>, keys: &Keys) -> DeleteResult {
        if event.matches(&keys.general.abort) {
            return DeleteResult::Close;
        }

        if event.matches(&keys.general.confirm) && self.name.text() == self.id.name {
            return DeleteResult::Delete(self.id.clone());
        }

        if util::handle_editor_input_event(&mut self.name, event, keys, util::is_room_char) {
            return DeleteResult::Handled;
        }

        DeleteResult::Unhandled
    }

    pub fn widget(&mut self) -> impl Widget<UiError> + '_ {
        let warn_style = Style::new().bold().red();
        let room_style = Style::new().bold().blue();
        let text = Styled::new_plain("Are you sure you want to delete ")
            .then("&", room_style)
            .then(&self.id.name, room_style)
            .then_plain(" on the ")
            .then(&self.id.domain, Style::new().grey())
            .then_plain(" server?\n\n")
            .then_plain("This will delete the entire room history from your vault. ")
            .then_plain("To shrink your vault afterwards, run ")
            .then("cove gc", Style::new().italic().grey())
            .then_plain(".\n\n")
            .then_plain("To confirm the deletion, ")
            .then_plain("enter the full name of the room and press enter:");

        let inner = Join2::vertical(
            // The Join prevents the text from filling up the entire available
            // space if the editor is wider than the text.
            Join2::horizontal(
                Text::new(text)
                    .resize()
                    .with_max_width(54)
                    .segment()
                    .with_growing(false),
                Empty::new().segment(),
            )
            .segment(),
            Join2::horizontal(
                Text::new(("&", room_style)).segment().with_fixed(true),
                self.name
                    .widget()
                    .with_highlight(|s| Styled::new(s, room_style))
                    .segment(),
            )
            .segment(),
        );

        Popup::new(inner, "Delete room").with_border_style(warn_style)
    }
}
