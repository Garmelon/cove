// TODO Remove mut in &mut Frame wherever applicable in this entire module

mod indent;
mod time;

use crossterm::style::{ContentStyle, Stylize};
use toss::frame::Frame;

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

pub fn msg<M: Msg + ChatMsg>(highlighted: bool, indent: usize, msg: &M) -> BoxedWidget {
    let (nick, content) = msg.styled();
    HJoin::new(vec![
        Segment::new(
            Padding::new(time::widget(Some(msg.time()), highlighted))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, highlighted)),
        Segment::new(Layer::new(vec![
            Indent::new(1, false).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        // TODO Minimum content width
        // TODO Minimizing and maximizing messages
        Segment::new(Text::new(content).wrap(true)).priority(1),
    ])
    .into()
}

pub fn msg_placeholder(highlighted: bool, indent: usize) -> BoxedWidget {
    HJoin::new(vec![
        Segment::new(
            Padding::new(time::widget(None, highlighted))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, highlighted)),
        Segment::new(Text::new((PLACEHOLDER, style_placeholder()))),
    ])
    .into()
}

pub fn editor<M: ChatMsg>(
    frame: &mut Frame,
    indent: usize,
    nick: &str,
    editor: &EditorState,
) -> (BoxedWidget, usize) {
    let (nick, content) = M::edit(nick, &editor.text());
    let editor = editor.widget().highlight(|_| content);
    let cursor_row = editor.cursor_row(frame);

    let widget = HJoin::new(vec![
        Segment::new(
            Padding::new(time::widget(None, true))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, true)),
        Segment::new(Layer::new(vec![
            Indent::new(1, false).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        Segment::new(editor).priority(1),
    ])
    .into();

    (widget, cursor_row)
}

pub fn pseudo<M: ChatMsg>(indent: usize, nick: &str, editor: &EditorState) -> BoxedWidget {
    let (nick, content) = M::edit(nick, &editor.text());

    HJoin::new(vec![
        Segment::new(
            Padding::new(time::widget(None, true))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, true)),
        Segment::new(Layer::new(vec![
            Indent::new(1, false).into(),
            Padding::new(Text::new(nick)).right(1).into(),
        ])),
        Segment::new(Text::new(content).wrap(true)).priority(1),
    ])
    .into()
}
