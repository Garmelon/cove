use std::borrow::Cow;
use std::iter;

use crossterm::style::{Color, ContentStyle, Stylize};
use euphoxide::api::{NickEvent, SessionId, SessionType, SessionView, UserId};
use euphoxide::conn::{Joined, SessionInfo};
use toss::styled::Styled;

use crate::euph;
use crate::ui::widgets::background::Background;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::list::{List, ListState};
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

pub fn widget(state: &ListState<SessionId>, joined: &Joined, focused: bool) -> BoxedWidget {
    let mut list = state.widget().focus(focused);
    render_rows(&mut list, joined);
    list.into()
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

fn render_rows(list: &mut List<SessionId>, joined: &Joined) {
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

    render_section(list, "People", &people, &joined.session);
    render_section(list, "Bots", &bots, &joined.session);
    render_section(list, "Lurkers", &lurkers, &joined.session);
    render_section(list, "Nurkers", &nurkers, &joined.session);
}

fn render_section(
    list: &mut List<SessionId>,
    name: &str,
    sessions: &[HalfSession],
    own_session: &SessionView,
) {
    if sessions.is_empty() {
        return;
    }

    let heading_style = ContentStyle::new().bold();

    if !list.is_empty() {
        list.add_unsel(Empty::new());
    }

    let row = Styled::new_plain(" ")
        .then(name, heading_style)
        .then_plain(format!(" ({})", sessions.len()));
    list.add_unsel(Text::new(row));

    for session in sessions {
        render_row(list, session, own_session);
    }
}

fn render_row(list: &mut List<SessionId>, session: &HalfSession, own_session: &SessionView) {
    let (name, style, style_inv, perms_style_inv) = if session.name.is_empty() {
        let name = "lurk";
        let style = ContentStyle::default().grey();
        let style_inv = ContentStyle::default().black().on_grey();
        (Cow::Borrowed(name), style, style_inv, style_inv)
    } else {
        let name = &session.name as &str;
        let (r, g, b) = euph::nick_color(name);
        let color = Color::Rgb { r, g, b };
        let style = ContentStyle::default().bold().with(color);
        let style_inv = ContentStyle::default().bold().black().on(color);
        let perms_style_inv = ContentStyle::default().black().on(color);
        (euph::EMOJI.replace(name), style, style_inv, perms_style_inv)
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

    let normal = Styled::new_plain(owner)
        .then(&name, style)
        .then_plain(perms);
    let selected = Styled::new_plain(owner)
        .then(name, style_inv)
        .then(perms, perms_style_inv);
    list.add_sel(
        session.session_id.clone(),
        Text::new(normal),
        Background::new(Text::new(selected)).style(style_inv),
    );
}
