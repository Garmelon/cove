use crossterm::style::{ContentStyle, Stylize};
use time::OffsetDateTime;
use toss::styled::Styled;

use crate::store::Msg;
use crate::ui::ChatMsg;

use super::api::{Snowflake, Time};
use super::util;

#[derive(Debug, Clone)]
pub struct SmallMessage {
    pub id: Snowflake,
    pub parent: Option<Snowflake>,
    pub time: Time,
    pub nick: String,
    pub content: String,
}

fn as_me(content: &str) -> Option<&str> {
    content.strip_prefix("/me")
}

fn style_me() -> ContentStyle {
    ContentStyle::default().grey().italic()
}

fn styled_nick(nick: &str) -> Styled {
    Styled::new_plain("[")
        .then(nick, util::nick_style(nick))
        .then_plain("]")
}

fn styled_nick_me(nick: &str) -> Styled {
    let style = style_me();
    Styled::new("*", style).then(nick, util::nick_style(nick).italic())
}

fn styled_content(content: &str) -> Styled {
    Styled::new_plain(content.trim())
}

fn styled_content_me(content: &str) -> Styled {
    let style = style_me();
    Styled::new(content.trim(), style).then("*", style)
}

fn styled_editor_content(content: &str) -> Styled {
    Styled::new_plain(content)
}

impl Msg for SmallMessage {
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

impl ChatMsg for SmallMessage {
    fn time(&self) -> OffsetDateTime {
        self.time.0
    }

    fn styled(&self) -> (Styled, Styled) {
        Self::pseudo(&self.nick, &self.content)
    }

    fn edit(nick: &str, content: &str) -> (Styled, Styled) {
        (styled_nick(nick), styled_editor_content(content))
    }

    fn pseudo(nick: &str, content: &str) -> (Styled, Styled) {
        if let Some(content) = as_me(content) {
            (styled_nick_me(nick), styled_content_me(content))
        } else {
            (styled_nick(nick), styled_content(content))
        }
    }
}
