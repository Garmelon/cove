// TODO Remove mut in &mut Frame wherever applicable in this entire module

mod indent;
mod time;

use crossterm::style::{ContentStyle, Stylize};

use crate::store::Msg;
use crate::ui::widgets::join::{HJoin, Segment};
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

use self::indent::Indent;

pub const PLACEHOLDER: &str = "[...]";

pub fn style_placeholder() -> ContentStyle {
    ContentStyle::default().dark_grey()
}

pub fn msg<M: Msg>(highlighted: bool, indent: usize, msg: &M) -> BoxedWidget {
    HJoin::new(vec![
        Segment::new(
            Padding::new(time::widget(Some(msg.time()), highlighted))
                .stretch(true)
                .right(1),
        ),
        Segment::new(Indent::new(indent, highlighted)),
        Segment::new(Padding::new(Text::new(msg.nick())).right(1)),
        // TODO Minimum content width
        // TODO Minimizing and maximizing messages
        Segment::new(Text::new(msg.content()).wrap(true)).priority(1),
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
