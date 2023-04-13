use std::convert::Infallible;

use crossterm::style::Stylize;
use toss::widgets::{BoxedAsync, EditorState, Join2, Join4, Join5, Text};
use toss::{Style, Styled, WidgetExt};

use crate::store::Msg;
use crate::ui::chat2::widgets::{Indent, Seen, Time};
use crate::ui::ChatMsg;

pub const PLACEHOLDER: &str = "[...]";

pub fn style_placeholder() -> Style {
    Style::new().dark_grey()
}

fn style_time(highlighted: bool) -> Style {
    if highlighted {
        Style::new().black().on_white()
    } else {
        Style::new().grey()
    }
}

fn style_indent(highlighted: bool) -> Style {
    if highlighted {
        Style::new().black().on_white()
    } else {
        Style::new().dark_grey()
    }
}

fn style_info() -> Style {
    Style::new().italic().dark_grey()
}

fn style_editor_highlight() -> Style {
    Style::new().black().on_cyan()
}

fn style_pseudo_highlight() -> Style {
    Style::new().black().on_yellow()
}

pub fn msg<M: Msg + ChatMsg>(
    highlighted: bool,
    indent: usize,
    msg: &M,
    folded_info: Option<usize>,
) -> BoxedAsync<'static, Infallible> {
    let (nick, mut content) = msg.styled();

    if let Some(amount) = folded_info {
        content = content
            .then_plain("\n")
            .then(format!("[{amount} more]"), style_info());
    }

    Join5::horizontal(
        Seen::new(msg.seen()).segment().with_fixed(true),
        Time::new(Some(msg.time()), style_time(highlighted))
            .padding()
            .with_right(1)
            .with_stretch(true)
            .segment()
            .with_fixed(true),
        Indent::new(indent, style_indent(highlighted))
            .segment()
            .with_fixed(true),
        Join2::vertical(
            Text::new(nick)
                .padding()
                .with_right(1)
                .segment()
                .with_fixed(true),
            Indent::new(1, style_indent(false)).segment(),
        )
        .segment()
        .with_fixed(true),
        // TODO Minimum content width
        // TODO Minimizing and maximizing messages
        Text::new(content).segment(),
    )
    .boxed_async()
}

pub fn msg_placeholder(
    highlighted: bool,
    indent: usize,
    folded_info: Option<usize>,
) -> BoxedAsync<'static, Infallible> {
    let mut content = Styled::new(PLACEHOLDER, style_placeholder());

    if let Some(amount) = folded_info {
        content = content
            .then_plain("\n")
            .then(format!("[{amount} more]"), style_info());
    }

    Join4::horizontal(
        Seen::new(true).segment().with_fixed(true),
        Time::new(None, style_time(highlighted))
            .padding()
            .with_right(1)
            .with_stretch(true)
            .segment()
            .with_fixed(true),
        Indent::new(indent, style_indent(highlighted))
            .segment()
            .with_fixed(true),
        Text::new(content).segment(),
    )
    .boxed_async()
}

pub fn editor<'a, M: ChatMsg>(
    indent: usize,
    nick: &str,
    editor: &'a mut EditorState,
) -> BoxedAsync<'a, Infallible> {
    let (nick, content) = M::edit(nick, editor.text());
    let editor = editor.widget().with_highlight(|_| content);

    Join5::horizontal(
        Seen::new(true).segment().with_fixed(true),
        Time::new(None, style_editor_highlight())
            .padding()
            .with_right(1)
            .with_stretch(true)
            .segment()
            .with_fixed(true),
        Indent::new(indent, style_editor_highlight())
            .segment()
            .with_fixed(true),
        Join2::vertical(
            Text::new(nick)
                .padding()
                .with_right(1)
                .segment()
                .with_fixed(true),
            Indent::new(1, style_indent(false)).segment(),
        )
        .segment()
        .with_fixed(true),
        editor.segment(),
    )
    .boxed_async()
}

pub fn pseudo<'a, M: ChatMsg>(
    indent: usize,
    nick: &str,
    editor: &'a mut EditorState,
) -> BoxedAsync<'a, Infallible> {
    let (nick, content) = M::edit(nick, editor.text());

    Join5::horizontal(
        Seen::new(true).segment().with_fixed(true),
        Time::new(None, style_pseudo_highlight())
            .padding()
            .with_right(1)
            .with_stretch(true)
            .segment()
            .with_fixed(true),
        Indent::new(indent, style_pseudo_highlight())
            .segment()
            .with_fixed(true),
        Join2::vertical(
            Text::new(nick)
                .padding()
                .with_right(1)
                .segment()
                .with_fixed(true),
            Indent::new(1, style_indent(false)).segment(),
        )
        .segment()
        .with_fixed(true),
        Text::new(content).segment(),
    )
    .boxed_async()
}
