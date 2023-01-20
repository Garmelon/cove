use crossterm::style::{Color, ContentStyle, Stylize};
use euphoxide::api::{MessageId, Snowflake, Time};
use time::OffsetDateTime;
use toss::styled::Styled;

use crate::store::Msg;
use crate::ui::ChatMsg;

use super::util;

fn nick_char(ch: char) -> bool {
    // Closely following the heim mention regex:
    // https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/stores/chat.js#L14-L15
    match ch {
        ',' | '.' | '!' | '?' | ';' | '&' | '<' | '\'' | '"' => false,
        _ => !ch.is_whitespace(),
    }
}

fn nick_char_(ch: Option<&char>) -> bool {
    ch.filter(|c| nick_char(**c)).is_some()
}

fn room_char(ch: char) -> bool {
    // Basically just \w, see also
    // https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/ui/MessageText.js#L66
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn room_char_(ch: Option<&char>) -> bool {
    ch.filter(|c| room_char(**c)).is_some()
}

// TODO Allocate less?
fn highlight_content(content: &str, base_style: ContentStyle) -> Styled {
    let mut result = Styled::default();
    let mut current = String::new();
    let mut chars = content.chars().peekable();
    let mut possible_room_or_mention = true;

    while let Some(char) = chars.next() {
        match char {
            '@' if possible_room_or_mention && nick_char_(chars.peek()) => {
                result = result.then(&current, base_style);
                current.clear();

                let mut nick = String::new();
                while let Some(ch) = chars.peek() {
                    if nick_char(*ch) {
                        nick.push(*ch);
                    } else {
                        break;
                    }
                    chars.next();
                }

                let (r, g, b) = util::nick_color(&nick);
                let style = base_style.with(Color::Rgb { r, g, b }).bold();
                result = result.then("@", style).then(nick, style);
            }
            '&' if possible_room_or_mention && room_char_(chars.peek()) => {
                result = result.then(&current, base_style);
                current.clear();

                let mut room = "&".to_string();
                while let Some(ch) = chars.peek() {
                    if room_char(*ch) {
                        room.push(*ch);
                    } else {
                        break;
                    }
                    chars.next();
                }

                let style = base_style.blue().bold();
                result = result.then(room, style);
            }
            _ => current.push(char),
        }

        // More permissive than the heim web client
        possible_room_or_mention = !char.is_alphanumeric();
    }

    result = result.then(current, base_style);

    result
}

#[derive(Debug, Clone)]
pub struct SmallMessage {
    pub id: MessageId,
    pub parent: Option<MessageId>,
    pub time: Time,
    pub nick: String,
    pub content: String,
    pub seen: bool,
}

fn as_me(content: &str) -> Option<&str> {
    content.strip_prefix("/me")
}

fn style_me() -> ContentStyle {
    ContentStyle::default().grey().italic()
}

fn styled_nick(nick: &str) -> Styled {
    Styled::new_plain("[")
        .and_then(util::style_nick(nick, ContentStyle::default()))
        .then_plain("]")
}

fn styled_nick_me(nick: &str) -> Styled {
    let style = style_me();
    Styled::new("*", style).and_then(util::style_nick(nick, style))
}

fn styled_content(content: &str) -> Styled {
    highlight_content(content.trim(), ContentStyle::default())
}

fn styled_content_me(content: &str) -> Styled {
    let style = style_me();
    highlight_content(content.trim(), style).then("*", style)
}

fn styled_editor_content(content: &str) -> Styled {
    let style = if as_me(content).is_some() {
        style_me()
    } else {
        ContentStyle::default()
    };
    highlight_content(content, style)
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
