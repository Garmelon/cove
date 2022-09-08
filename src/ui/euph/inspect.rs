use crossterm::style::{ContentStyle, Stylize};
use euphoxide::api::Message;
use toss::styled::Styled;

use crate::ui::input::{key, InputEvent, KeyBindingsList};
use crate::ui::widgets::popup::Popup;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

macro_rules! line {
    ( $text:ident, $name:expr, $val:expr ) => {
        $text = $text
            .then($name, ContentStyle::default().cyan())
            .then_plain(format!(" {}\n", $val));
    };
    ( $text:ident, $name:expr, $val:expr, debug ) => {
        $text = $text
            .then($name, ContentStyle::default().cyan())
            .then_plain(format!(" {:?}\n", $val));
    };
    ( $text:ident, $name:expr, $val:expr, optional ) => {
        if let Some(val) = $val {
            $text = $text
                .then($name, ContentStyle::default().cyan())
                .then_plain(format!(" {val}\n"));
        } else {
            $text = $text
                .then($name, ContentStyle::default().cyan())
                .then_plain(" ")
                .then("none", ContentStyle::default().italic().grey())
                .then_plain("\n");
        }
    };
    ( $text:ident, $name:expr, $val:expr, yes or no ) => {
        $text = $text
            .then($name, ContentStyle::default().cyan())
            .then_plain(if $val { " yes\n" } else { " no\n" });
    };
}

pub fn message_widget(msg: &Message) -> BoxedWidget {
    let heading_style = ContentStyle::default().bold();

    let mut text = Styled::new("Message", heading_style).then_plain("\n");
    line!(text, "id", msg.id);
    line!(text, "parent", msg.parent, optional);
    line!(text, "previous_edit_id", msg.previous_edit_id, optional);
    line!(text, "time", msg.time.0);
    line!(text, "encryption_key_id", &msg.encryption_key_id, optional);
    line!(text, "edited", msg.edited.map(|t| t.0), optional);
    line!(text, "deleted", msg.deleted.map(|t| t.0), optional);
    line!(text, "truncated", msg.truncated, yes or no);
    text = text.then_plain("\n");

    text = text.then("Sender", heading_style).then_plain("\n");
    line!(text, "id", msg.sender.id);
    line!(text, "name", msg.sender.name);
    line!(text, "name (raw)", msg.sender.name, debug);
    line!(text, "server_id", msg.sender.server_id);
    line!(text, "server_era", msg.sender.server_era);
    line!(text, "session_id", msg.sender.session_id);
    line!(text, "is_staff", msg.sender.is_staff, yes or no);
    line!(text, "is_manager", msg.sender.is_manager, yes or no);
    line!(
        text,
        "client_address",
        msg.sender.client_address.as_ref(),
        optional
    );
    line!(
        text,
        "real_client_address",
        msg.sender.real_client_address.as_ref(),
        optional
    );

    Popup::new(Text::new(text)).title("Inspect message").build()
}

pub fn list_key_bindings(bindings: &mut KeyBindingsList) {
    bindings.binding("esc", "close");
}

pub enum EventResult {
    NotHandled,
    Close,
}

pub fn handle_input_event(event: &InputEvent) -> EventResult {
    match event {
        key!(Esc) => EventResult::Close,
        _ => EventResult::NotHandled,
    }
}
