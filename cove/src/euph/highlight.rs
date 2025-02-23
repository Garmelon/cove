use std::ops::Range;

use crossterm::style::Stylize;
use toss::{Style, Styled};

use crate::euph::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanType {
    Mention,
    Room,
    Emoji,
}

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

struct SpanFinder<'a> {
    content: &'a str,

    span: Option<(SpanType, usize)>,
    room_or_mention_possible: bool,

    result: Vec<(SpanType, Range<usize>)>,
}

impl<'a> SpanFinder<'a> {
    fn is_valid_span(&self, span: SpanType, range: Range<usize>) -> bool {
        let text = &self.content[range.start..range.end];
        match span {
            SpanType::Mention => range.len() > 1 && text.starts_with('@'),
            SpanType::Room => range.len() > 1 && text.starts_with('&'),
            SpanType::Emoji => {
                if range.len() <= 2 {
                    return false;
                }

                let Some(name) = Some(text)
                    .and_then(|it| it.strip_prefix(':'))
                    .and_then(|it| it.strip_suffix(':'))
                else {
                    return false;
                };

                util::EMOJI.get(name).is_some()
            }
        }
    }

    fn close_span(&mut self, end: usize) {
        let Some((span, start)) = self.span else {
            return;
        };
        if self.is_valid_span(span, start..end) {
            self.result.push((span, start..end));
        }
        self.span = None;
    }

    fn open_span(&mut self, span: SpanType, start: usize) {
        self.close_span(start);
        self.span = Some((span, start))
    }

    fn step(&mut self, idx: usize, char: char) {
        match (char, self.span) {
            ('@', _) if self.room_or_mention_possible => self.open_span(SpanType::Mention, idx),
            ('&', _) if self.room_or_mention_possible => self.open_span(SpanType::Room, idx),
            (':', None) => self.open_span(SpanType::Emoji, idx),
            (':', Some((SpanType::Emoji, _))) => self.close_span(idx + 1),
            (c, Some((SpanType::Mention, _))) if !nick_char(c) => self.close_span(idx),
            (c, Some((SpanType::Room, _))) if !room_char(c) => self.close_span(idx),
            _ => {}
        }

        // More permissive than the heim web client
        self.room_or_mention_possible = !char.is_alphanumeric();
    }

    fn find(content: &'a str) -> Vec<(SpanType, Range<usize>)> {
        let mut this = Self {
            content,
            span: None,
            room_or_mention_possible: true,
            result: vec![],
        };

        for (idx, char) in content.char_indices() {
            this.step(idx, char);
        }

        this.close_span(content.len());

        this.result
    }
}

pub fn find_spans(content: &str) -> Vec<(SpanType, Range<usize>)> {
    SpanFinder::find(content)
}

/// Highlight spans in a string.
///
/// The list of spans must be non-overlapping and in ascending order.
///
/// If `exact` is specified, colon-delimited emoji are not replaced with their
/// unicode counterparts.
pub fn apply_spans(
    content: &str,
    spans: &[(SpanType, Range<usize>)],
    base: Style,
    exact: bool,
) -> Styled {
    let mut result = Styled::default();
    let mut i = 0;

    for (span, range) in spans {
        assert!(i <= range.start);
        assert!(range.end <= content.len());

        if i < range.start {
            result = result.then(&content[i..range.start], base);
        }

        let text = &content[range.start..range.end];
        result = match span {
            SpanType::Mention if exact => result.and_then(util::style_nick_exact(text, base)),
            SpanType::Mention => result.and_then(util::style_nick(text, base)),
            SpanType::Room => result.then(text, base.blue().bold()),
            SpanType::Emoji if exact => result.then(text, base.magenta()),
            SpanType::Emoji => {
                let name = text.strip_prefix(':').unwrap_or(text);
                let name = name.strip_suffix(':').unwrap_or(name);
                if let Some(Some(replacement)) = util::EMOJI.get(name) {
                    result.then(replacement, base)
                } else {
                    result.then(name, base.magenta())
                }
            }
        };

        i = range.end;
    }

    if i < content.len() {
        result = result.then(&content[i..], base);
    }

    result
}

/// Highlight an euphoria message's content.
///
/// If `exact` is specified, colon-delimited emoji are not replaced with their
/// unicode counterparts.
pub fn highlight(content: &str, base: Style, exact: bool) -> Styled {
    apply_spans(content, &find_spans(content), base, exact)
}

#[cfg(test)]
mod tests {

    use crate::euph::SpanType;

    use super::find_spans;

    #[test]
    fn mentions() {
        assert_eq!(find_spans("@foo"), vec![(SpanType::Mention, 0..4)]);
        assert_eq!(find_spans("a @foo b"), vec![(SpanType::Mention, 2..6)]);
        assert_eq!(find_spans("@@foo@"), vec![(SpanType::Mention, 1..6)]);
        assert_eq!(find_spans("a @b@c d"), vec![(SpanType::Mention, 2..6)]);
        assert_eq!(
            find_spans("a @b @c d"),
            vec![(SpanType::Mention, 2..4), (SpanType::Mention, 5..7)]
        );
    }

    #[test]
    fn rooms() {
        assert_eq!(find_spans("&foo"), vec![(SpanType::Room, 0..4)]);
        assert_eq!(find_spans("a &foo b"), vec![(SpanType::Room, 2..6)]);
        assert_eq!(find_spans("&&foo&"), vec![(SpanType::Room, 1..5)]);
        assert_eq!(find_spans("a &b&c d"), vec![(SpanType::Room, 2..4)]);
        assert_eq!(
            find_spans("a &b &c d"),
            vec![(SpanType::Room, 2..4), (SpanType::Room, 5..7)]
        );
    }
}
