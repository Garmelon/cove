use std::mem;

use crossterm::style::Stylize;
use euphoxide::api::{MessageId, Snowflake, Time};
use jiff::Timestamp;
use toss::{Style, Styled};

use crate::store::Msg;
use crate::ui::ChatMsg;

use super::util;

fn nick_char(ch: char) -> bool {
    // Closely following the heim mention regex:
    // https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/stores/chat.js#L14-L15
    // `>` has been experimentally confirmed to delimit mentions as well.
    match ch {
        ',' | '.' | '!' | '?' | ';' | '&' | '<' | '>' | '\'' | '"' => false,
        _ => !ch.is_whitespace(),
    }
}

fn room_char(ch: char) -> bool {
    // Basically just \w, see also
    // https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/ui/MessageText.js#L66
    ch.is_ascii_alphanumeric() || ch == '_'
}

enum Span {
    Nothing,
    Mention,
    Room,
    Emoji,
}

struct Highlighter<'a> {
    content: &'a str,
    base_style: Style,
    exact: bool,

    span: Span,
    span_start: usize,
    room_or_mention_possible: bool,

    result: Styled,
}

impl<'a> Highlighter<'a> {
    /// Does *not* guarantee `self.span_start == idx` after running!
    fn close_mention(&mut self, idx: usize) {
        let span_length = idx.saturating_sub(self.span_start);
        if span_length <= 1 {
            // We can repurpose the current span
            self.span = Span::Nothing;
            return;
        }

        let text = &self.content[self.span_start..idx]; // Includes @
        self.result = mem::take(&mut self.result).and_then(if self.exact {
            util::style_nick_exact(text, self.base_style)
        } else {
            util::style_nick(text, self.base_style)
        });

        self.span = Span::Nothing;
        self.span_start = idx;
    }

    /// Does *not* guarantee `self.span_start == idx` after running!
    fn close_room(&mut self, idx: usize) {
        let span_length = idx.saturating_sub(self.span_start);
        if span_length <= 1 {
            // We can repurpose the current span
            self.span = Span::Nothing;
            return;
        }

        self.result = mem::take(&mut self.result).then(
            &self.content[self.span_start..idx],
            self.base_style.blue().bold(),
        );

        self.span = Span::Nothing;
        self.span_start = idx;
    }

    // Warning: `idx` is the index of the closing colon.
    fn close_emoji(&mut self, idx: usize) {
        let name = &self.content[self.span_start + 1..idx];
        if let Some(replace) = util::EMOJI.get(name) {
            match replace {
                Some(replace) if !self.exact => {
                    self.result = mem::take(&mut self.result).then(replace, self.base_style);
                }
                _ => {
                    let text = &self.content[self.span_start..=idx];
                    let style = self.base_style.magenta();
                    self.result = mem::take(&mut self.result).then(text, style);
                }
            }

            self.span = Span::Nothing;
            self.span_start = idx + 1;
        } else {
            self.close_plain(idx);
            self.span = Span::Emoji;
        }
    }

    /// Guarantees `self.span_start == idx` after running.
    fn close_plain(&mut self, idx: usize) {
        if self.span_start == idx {
            // Span has length 0
            return;
        }

        self.result =
            mem::take(&mut self.result).then(&self.content[self.span_start..idx], self.base_style);

        self.span = Span::Nothing;
        self.span_start = idx;
    }

    fn close_span_before_current_char(&mut self, idx: usize, char: char) {
        match self.span {
            Span::Mention if !nick_char(char) => self.close_mention(idx),
            Span::Room if !room_char(char) => self.close_room(idx),
            Span::Emoji if char == '&' || char == '@' => {
                self.span = Span::Nothing;
            }
            _ => {}
        }
    }

    fn update_span_with_current_char(&mut self, idx: usize, char: char) {
        match self.span {
            Span::Nothing if char == '@' && self.room_or_mention_possible => {
                self.close_plain(idx);
                self.span = Span::Mention;
            }
            Span::Nothing if char == '&' && self.room_or_mention_possible => {
                self.close_plain(idx);
                self.span = Span::Room;
            }
            Span::Nothing if char == ':' => {
                self.close_plain(idx);
                self.span = Span::Emoji;
            }
            Span::Emoji if char == ':' => self.close_emoji(idx),
            _ => {}
        }
    }

    fn close_final_span(&mut self) {
        let idx = self.content.len();
        if self.span_start >= idx {
            return; // Span has no contents
        }

        match self.span {
            Span::Mention => self.close_mention(idx),
            Span::Room => self.close_room(idx),
            _ => {}
        }

        self.close_plain(idx);
    }

    fn step(&mut self, idx: usize, char: char) {
        if self.span_start < idx {
            self.close_span_before_current_char(idx, char);
        }

        self.update_span_with_current_char(idx, char);

        // More permissive than the heim web client
        self.room_or_mention_possible = !char.is_alphanumeric();
    }

    fn highlight(content: &'a str, base_style: Style, exact: bool) -> Styled {
        let mut this = Self {
            content: if exact { content } else { content.trim() },
            base_style,
            exact,
            span: Span::Nothing,
            span_start: 0,
            room_or_mention_possible: true,
            result: Styled::default(),
        };

        for (idx, char) in (if exact { content } else { content.trim() }).char_indices() {
            this.step(idx, char);
        }

        this.close_final_span();

        this.result
    }
}

fn highlight_content(content: &str, base_style: Style, exact: bool) -> Styled {
    Highlighter::highlight(content, base_style, exact)
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

fn style_me() -> Style {
    Style::new().grey().italic()
}

fn styled_nick(nick: &str) -> Styled {
    Styled::new_plain("[")
        .and_then(util::style_nick(nick, Style::new()))
        .then_plain("]")
}

fn styled_nick_me(nick: &str) -> Styled {
    let style = style_me();
    Styled::new("*", style).and_then(util::style_nick(nick, style))
}

fn styled_content(content: &str) -> Styled {
    highlight_content(content.trim(), Style::new(), false)
}

fn styled_content_me(content: &str) -> Styled {
    let style = style_me();
    highlight_content(content.trim(), style, false).then("*", style)
}

fn styled_editor_content(content: &str) -> Styled {
    let style = if as_me(content).is_some() {
        style_me()
    } else {
        Style::new()
    };
    highlight_content(content, style, true)
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
