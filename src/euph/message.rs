use crossterm::style::{ContentStyle, Stylize};
use time::OffsetDateTime;
use toss::styled::Styled;

use crate::store::Msg;
use crate::ui::ChatMsg;

use super::api::{Snowflake, Time};
use super::util;

#[derive(Debug, Clone)]
pub struct Message {
    pub id: Snowflake,
    pub parent: Option<Snowflake>,
    pub time: Time,
    pub nick: String,
    pub content: String,
}

fn styled_nick(nick: &str) -> Styled {
    Styled::new_plain("[")
        .then(nick, util::nick_style(nick))
        .then_plain("]")
}

fn styled_content(content: &str) -> Styled {
    Styled::new_plain(content.trim())
}

impl Msg for Message {
    type Id = Snowflake;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn parent(&self) -> Option<Self::Id> {
        self.parent
    }

    fn last_possible_id() -> Self::Id {
        Snowflake::MAX
    }
}

impl ChatMsg for Message {
    fn time(&self) -> OffsetDateTime {
        self.time.0
    }

    fn styled(&self) -> (Styled, Styled) {
        Self::pseudo(&self.nick, &self.content)
    }

    fn edit(nick: &str, content: &str) -> (Styled, Styled) {
        (styled_nick(nick), styled_content(content))
    }

    fn pseudo(nick: &str, content: &str) -> (Styled, Styled) {
        (styled_nick(nick), styled_content(content))
    }
}
