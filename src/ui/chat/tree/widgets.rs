mod indent;
mod seen;
mod time;

use crossterm::style::{ContentStyle, Stylize};
use toss::styled::Styled;
use toss::widthdb::WidthDb;

use super::super::ChatMsg;
use crate::store::Msg;
use crate::ui::widgets::editor::EditorState;
use crate::ui::widgets::join::{HJoin, Segment};
use crate::ui::widgets::layer::Layer;
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

use self::indent::Indent;

pub const PLACEHOLDER: &str = "[...]";

pub fn style_placeholder() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

fn style_time(highlighted: bool) -> ContentStyle {
    if highlighted {
        ContentStyle::default().black().on_white()
    } else {
        ContentStyle::default().grey()
    }
}

fn style_indent(highlighted: bool) -> ContentStyle {
    if highlighted {
        ContentStyle::default().black().on_white()
    } else {
        ContentStyle::default().dark_grey()
    }
}

fn style_info() -> ContentStyle {
    ContentStyle::default().italic().dark_grey()
}

fn style_editor_highlight() -> ContentStyle {
    ContentStyle::default().black().on_cyan()
}

fn style_pseudo_highlight() -> ContentStyle {
    ContentStyle::default().black().on_yellow()
}

pub fn msg<M: Msg + ChatMsg>(
    highlighted: bool,
    indent: usize,
    msg: &M,
    folded_info: Option<usize>,
) -> BoxedWidget {
    let (nick, mut content) = msg.styled();

    if let Some(amount) = folded_info {
        content = content
            .then_plain("\n")
            .then(format!("[{amount} more]"), style_info());
    }

    HJoin::new(vec![
        Segment::new(seen::widget(msg.seen())),
        Segment::new(
            Padding::new(time::widget(Some(msg.time()), style_time(highlighted)))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, style_indent(highlighted))),
        Segment::new(Layer::new(vec![
            Indent::new(1, style_indent(false)).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        // TODO Minimum content width
        // TODO Minimizing and maximizing messages
        Segment::new(Text::new(content).wrap(true)).priority(1),
    ])
    .into()
}

pub fn msg_placeholder(
    highlighted: bool,
    indent: usize,
    folded_info: Option<usize>,
) -> BoxedWidget {
    let mut content = Styled::new(PLACEHOLDER, style_placeholder());

    if let Some(amount) = folded_info {
        content = content
            .then_plain("\n")
            .then(format!("[{amount} more]"), style_info());
    }

    HJoin::new(vec![
        Segment::new(seen::widget(true)),
        Segment::new(
            Padding::new(time::widget(None, style_time(highlighted)))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, style_indent(highlighted))),
        Segment::new(Text::new(content)),
    ])
    .into()
}

pub fn editor<M: ChatMsg>(
    widthdb: &mut WidthDb,
    indent: usize,
    nick: &str,
    editor: &EditorState,
) -> (BoxedWidget, usize) {
    let (nick, content) = M::edit(nick, &editor.text());
    let editor = editor.widget().highlight(|_| content);
    let cursor_row = editor.cursor_row(widthdb);

    let widget = HJoin::new(vec![
        Segment::new(seen::widget(true)),
        Segment::new(
            Padding::new(time::widget(None, style_editor_highlight()))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, style_editor_highlight())),
        Segment::new(Layer::new(vec![
            Indent::new(1, style_indent(false)).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        Segment::new(editor).priority(1).expanding(true),
    ])
    .into();

    (widget, cursor_row)
}

pub fn pseudo<M: ChatMsg>(indent: usize, nick: &str, editor: &EditorState) -> BoxedWidget {
    let (nick, content) = M::edit(nick, &editor.text());

    HJoin::new(vec![
        Segment::new(seen::widget(true)),
        Segment::new(
            Padding::new(time::widget(None, style_pseudo_highlight()))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, style_pseudo_highlight())),
        Segment::new(Layer::new(vec![
            Indent::new(1, style_indent(false)).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        Segment::new(Text::new(content).wrap(true)).priority(1),
    ])
    .into()
}
