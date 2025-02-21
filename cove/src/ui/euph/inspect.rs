use cove_config::Keys;
use cove_input::InputEvent;
use crossterm::style::Stylize;
use euphoxide::{
    api::{Message, NickEvent, SessionView},
    conn::SessionInfo,
};
use toss::{Style, Styled, Widget, widgets::Text};

use crate::ui::{UiError, widgets::Popup};

use super::popup::PopupResult;

macro_rules! line {
    ( $text:ident, $name:expr, $val:expr ) => {
        $text = $text
            .then($name, Style::new().cyan())
            .then_plain(format!(" {}\n", $val));
    };
    ( $text:ident, $name:expr, $val:expr, debug ) => {
        $text = $text
            .then($name, Style::new().cyan())
            .then_plain(format!(" {:?}\n", $val));
    };
    ( $text:ident, $name:expr, $val:expr, optional ) => {
        if let Some(val) = $val {
            $text = $text
                .then($name, Style::new().cyan())
                .then_plain(format!(" {val}\n"));
        } else {
            $text = $text
                .then($name, Style::new().cyan())
                .then_plain(" ")
                .then("none", Style::new().italic().grey())
                .then_plain("\n");
        }
    };
    ( $text:ident, $name:expr, $val:expr, yes or no ) => {
        $text = $text.then($name, Style::new().cyan()).then_plain(if $val {
            " yes\n"
        } else {
            " no\n"
        });
    };
}

fn session_view_lines(mut text: Styled, session: &SessionView) -> Styled {
    line!(text, "id", session.id);
    line!(text, "name", session.name);
    line!(text, "name (raw)", session.name, debug);
    line!(text, "server_id", session.server_id);
    line!(text, "server_era", session.server_era);
    line!(text, "session_id", session.session_id.0);
    line!(text, "is_staff", session.is_staff, yes or no);
    line!(text, "is_manager", session.is_manager, yes or no);
    line!(
        text,
        "client_address",
        session.client_address.as_ref(),
        optional
    );
    line!(
        text,
        "real_client_address",
        session.real_client_address.as_ref(),
        optional
    );

    text
}

fn nick_event_lines(mut text: Styled, event: &NickEvent) -> Styled {
    line!(text, "id", event.id);
    line!(text, "name", event.to);
    line!(text, "name (raw)", event.to, debug);
    line!(text, "session_id", event.session_id.0);

    text
}

fn message_lines(mut text: Styled, msg: &Message) -> Styled {
    line!(text, "id", msg.id.0);
    line!(text, "parent", msg.parent.map(|p| p.0), optional);
    line!(text, "previous_edit_id", msg.previous_edit_id, optional);
    line!(text, "time", msg.time.0);
    line!(text, "encryption_key_id", &msg.encryption_key_id, optional);
    line!(text, "edited", msg.edited.map(|t| t.0), optional);
    line!(text, "deleted", msg.deleted.map(|t| t.0), optional);
    line!(text, "truncated", msg.truncated, yes or no);

    text
}

pub fn session_widget(session: &SessionInfo) -> impl Widget<UiError> + use<> {
    let heading_style = Style::new().bold();

    let text = match session {
        SessionInfo::Full(session) => {
            let text = Styled::new("Full session", heading_style).then_plain("\n");
            session_view_lines(text, session)
        }
        SessionInfo::Partial(event) => {
            let text = Styled::new("Partial session", heading_style).then_plain("\n");
            nick_event_lines(text, event)
        }
    };

    Popup::new(Text::new(text), "Inspect session")
}

pub fn message_widget(msg: &Message) -> impl Widget<UiError> + use<> {
    let heading_style = Style::new().bold();

    let mut text = Styled::new("Message", heading_style).then_plain("\n");

    text = message_lines(text, msg);

    text = text
        .then_plain("\n")
        .then("Sender", heading_style)
        .then_plain("\n");

    text = session_view_lines(text, &msg.sender);

    Popup::new(Text::new(text), "Inspect message")
}

pub fn handle_input_event(event: &mut InputEvent<'_>, keys: &Keys) -> PopupResult {
    if event.matches(&keys.general.abort) {
        return PopupResult::Close;
    }

    PopupResult::NotHandled
}
