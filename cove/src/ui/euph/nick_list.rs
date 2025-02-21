use std::iter;

use crossterm::style::{Color, Stylize};
use euphoxide::api::{NickEvent, SessionId, SessionType, SessionView, UserId};
use euphoxide::conn::{Joined, SessionInfo};
use toss::widgets::{Background, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use crate::euph;
use crate::ui::widgets::{ListBuilder, ListState};
use crate::ui::UiError;

pub fn widget<'a>(
    list: &'a mut ListState<SessionId>,
    joined: &Joined,
    focused: bool,
) -> impl Widget<UiError> + use<'a> {
    let mut list_builder = ListBuilder::new();
    render_rows(&mut list_builder, joined, focused);
    list_builder.build(list)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HalfSession {
    name: String,
    id: UserId,
    session_id: SessionId,
    is_staff: bool,
    is_manager: bool,
}

impl HalfSession {
    fn from_session_view(sess: &SessionView) -> Self {
        Self {
            name: sess.name.clone(),
            id: sess.id.clone(),
            session_id: sess.session_id.clone(),
            is_staff: sess.is_staff,
            is_manager: sess.is_manager,
        }
    }

    fn from_nick_event(nick: &NickEvent) -> Self {
        Self {
            name: nick.to.clone(),
            id: nick.id.clone(),
            session_id: nick.session_id.clone(),
            is_staff: false,
            is_manager: false,
        }
    }

    fn from_session_info(info: &SessionInfo) -> Self {
        match info {
            SessionInfo::Full(sess) => Self::from_session_view(sess),
            SessionInfo::Partial(nick) => Self::from_nick_event(nick),
        }
    }
}

fn render_rows(
    list_builder: &mut ListBuilder<'_, SessionId, Background<Text>>,
    joined: &Joined,
    focused: bool,
) {
    let mut people = vec![];
    let mut bots = vec![];
    let mut lurkers = vec![];
    let mut nurkers = vec![];

    let sessions = joined
        .listing
        .values()
        .map(HalfSession::from_session_info)
        .chain(iter::once(HalfSession::from_session_view(&joined.session)));
    for sess in sessions {
        match sess.id.session_type() {
            Some(SessionType::Bot) if sess.name.is_empty() => nurkers.push(sess),
            Some(SessionType::Bot) => bots.push(sess),
            _ if sess.name.is_empty() => lurkers.push(sess),
            _ => people.push(sess),
        }
    }

    people.sort_unstable();
    bots.sort_unstable();
    lurkers.sort_unstable();
    nurkers.sort_unstable();

    render_section(list_builder, "People", &people, &joined.session, focused);
    render_section(list_builder, "Bots", &bots, &joined.session, focused);
    render_section(list_builder, "Lurkers", &lurkers, &joined.session, focused);
    render_section(list_builder, "Nurkers", &nurkers, &joined.session, focused);
}

fn render_section(
    list_builder: &mut ListBuilder<'_, SessionId, Background<Text>>,
    name: &str,
    sessions: &[HalfSession],
    own_session: &SessionView,
    focused: bool,
) {
    if sessions.is_empty() {
        return;
    }

    let heading_style = Style::new().bold();

    if !list_builder.is_empty() {
        list_builder.add_unsel(Text::new("").background());
    }

    let row = Styled::new_plain(" ")
        .then(name, heading_style)
        .then_plain(format!(" ({})", sessions.len()));
    list_builder.add_unsel(Text::new(row).background());

    for session in sessions {
        render_row(list_builder, session, own_session, focused);
    }
}

fn render_row(
    list_builder: &mut ListBuilder<'_, SessionId, Background<Text>>,
    session: &HalfSession,
    own_session: &SessionView,
    focused: bool,
) {
    let (name, style, style_inv, perms_style_inv) = if session.name.is_empty() {
        let name = "lurk".to_string();
        let style = Style::new().grey();
        let style_inv = Style::new().black().on_grey();
        (name, style, style_inv, style_inv)
    } else {
        let name = &session.name as &str;
        let (r, g, b) = euph::nick_color(name);
        let name = euph::EMOJI.replace(name).to_string();
        let color = Color::Rgb { r, g, b };
        let style = Style::new().bold().with(color);
        let style_inv = Style::new().bold().black().on(color);
        let perms_style_inv = Style::new().black().on(color);
        (name, style, style_inv, perms_style_inv)
    };

    let perms = if session.is_staff {
        "!"
    } else if session.is_manager {
        "*"
    } else if session.id.session_type() == Some(SessionType::Account) {
        "~"
    } else {
        ""
    };

    let owner = if session.session_id == own_session.session_id {
        ">"
    } else {
        " "
    };

    list_builder.add_sel(session.session_id.clone(), move |selected| {
        if focused && selected {
            let text = Styled::new_plain(owner)
                .then(name, style_inv)
                .then(perms, perms_style_inv);
            Text::new(text).background().with_style(style_inv)
        } else {
            let text = Styled::new_plain(owner)
                .then(&name, style)
                .then_plain(perms);
            Text::new(text).background()
        }
    });
}
