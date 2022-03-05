use std::collections::HashSet;
use std::iter;

use cove_core::{Identity, Session};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};

use crate::cove::conn::Present;
use crate::ui::styles;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct UserInfo {
    nick: String,
    identity: Identity,
}

impl From<&Session> for UserInfo {
    fn from(s: &Session) -> Self {
        Self {
            nick: s.nick.clone(),
            identity: s.identity,
        }
    }
}

pub struct CoveUsers {
    users: Vec<UserInfo>,
}

impl CoveUsers {
    pub fn new(present: &Present) -> Self {
        let mut users: Vec<UserInfo> = iter::once(&present.session)
            .chain(present.others.values())
            .map(<&Session as Into<UserInfo>>::into)
            .collect();
        users.sort();
        Self { users }
    }
}

impl Widget for CoveUsers {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let sessions = self.users.len();
        let identities = self
            .users
            .iter()
            .map(|i| i.identity)
            .collect::<HashSet<_>>()
            .len();
        let title = format!("Users ({identities}/{sessions})");

        let mut lines = vec![Spans::from(Span::styled(title, styles::title()))];
        for user in self.users {
            // TODO Colour users based on identity
            lines.push(Spans::from(Span::from(user.nick)));
        }
        Paragraph::new(lines).render(area, buf);
    }
}
