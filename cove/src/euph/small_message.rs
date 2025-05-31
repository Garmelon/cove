use crossterm::style::Stylize;
use euphoxide::api::{MessageId, Snowflake, Time, UserId};
use jiff::Timestamp;
use toss::{Style, Styled};

use crate::{store::Msg, ui::ChatMsg};

use super::util;

#[derive(Debug, Clone)]
pub struct SmallMessage {
    pub id: MessageId,
    pub parent: Option<MessageId>,
    pub time: Time,
    pub user_id: UserId,
    pub nick: String,
    pub content: String,
    pub seen: bool,
}

fn as_me(content: &str) -> Option<&str> {
    content.strip_prefix("/me")
}

fn style_me() -> Style {
    Style::new().grey().italic()
}

fn styled_nick(nick: &str) -> Styled {
    Styled::new_plain("[")
        .and_then(super::style_nick(nick, Style::new()))
        .then_plain("]")
}

fn styled_nick_me(nick: &str) -> Styled {
    let style = style_me();
    Styled::new("*", style).and_then(super::style_nick(nick, style))
}

fn styled_content(content: &str) -> Styled {
    super::highlight(content.trim(), Style::new(), false)
}

fn styled_content_me(content: &str) -> Styled {
    let style = style_me();
    super::highlight(content.trim(), style, false).then("*", style)
}

fn styled_editor_content(content: &str) -> Styled {
    let style = if as_me(content).is_some() {
        style_me()
    } else {
        Style::new()
    };
    super::highlight(content, style, true)
}

impl Msg for SmallMessage {
    type Id = MessageId;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn parent(&self) -> Option<Self::Id> {
        self.parent
    }

    fn seen(&self) -> bool {
        self.seen
    }

    fn last_possible_id() -> Self::Id {
        MessageId(Snowflake::MAX)
    }

    fn nick_emoji(&self) -> Option<String> {
        Some(util::user_id_emoji(&self.user_id))
    }
}

impl ChatMsg for SmallMessage {
    fn time(&self) -> Option<Timestamp> {
        Some(self.time.as_timestamp())
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
