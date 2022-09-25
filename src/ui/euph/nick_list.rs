use std::iter;

use crossterm::style::{Color, ContentStyle, Stylize};
use euphoxide::api::{SessionType, SessionView};
use euphoxide::conn::Joined;
use toss::styled::Styled;

use crate::euph;
use crate::ui::widgets::background::Background;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::list::{List, ListState};
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

pub fn widget(state: &ListState<String>, joined: &Joined, focused: bool) -> BoxedWidget {
    // TODO Handle focus
    let mut list = state.widget();
    render_rows(&mut list, joined);
    list.into()
}

fn render_rows(list: &mut List<String>, joined: &Joined) {
    let mut people = vec![];
    let mut bots = vec![];
    let mut lurkers = vec![];
    let mut nurkers = vec![];

    let mut sessions = iter::once(&joined.session)
        .chain(joined.listing.values())
        .collect::<Vec<_>>();
    sessions.sort_unstable_by_key(|s| &s.name);
    for sess in sessions {
        match sess.id.session_type() {
            Some(SessionType::Bot) if sess.name.is_empty() => nurkers.push(sess),
            Some(SessionType::Bot) => bots.push(sess),
            _ if sess.name.is_empty() => lurkers.push(sess),
            _ => people.push(sess),
        }
    }

    people.sort_unstable_by_key(|s| (&s.name, &s.session_id));
    bots.sort_unstable_by_key(|s| (&s.name, &s.session_id));
    lurkers.sort_unstable_by_key(|s| &s.session_id);
    nurkers.sort_unstable_by_key(|s| &s.session_id);

    render_section(list, "People", &people, &joined.session);
    render_section(list, "Bots", &bots, &joined.session);
    render_section(list, "Lurkers", &lurkers, &joined.session);
    render_section(list, "Nurkers", &nurkers, &joined.session);
}

fn render_section(
    list: &mut List<String>,
    name: &str,
    sessions: &[&SessionView],
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

fn render_row(list: &mut List<String>, session: &SessionView, own_session: &SessionView) {
    let id = session.session_id.clone();

    let (name, style, style_inv) = if session.name.is_empty() {
        let name = "lurk";
        let style = ContentStyle::default().grey();
        let style_inv = ContentStyle::default().black().on_grey();
        (name, style, style_inv)
    } else {
        let name = &session.name as &str;
        let (r, g, b) = euph::nick_color(name);
        let color = Color::Rgb { r, g, b };
        let style = ContentStyle::default().bold().with(color);
        let style_inv = ContentStyle::default().bold().black().on(color);
        (name, style, style_inv)
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

    let normal = Styled::new_plain(owner).then(name, style).then_plain(perms);
    let selected = Styled::new_plain(owner)
        .then(name, style_inv)
        .then_plain(perms);
    list.add_sel(
        id,
        Text::new(normal),
        Background::new(Text::new(selected)).style(style_inv),
    );
}
