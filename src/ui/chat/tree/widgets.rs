use crate::store::Msg;
use crate::ui::widgets::join::{HJoin, Segment};
use crate::ui::widgets::padding::Padding;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

use super::time;

pub fn msg<M: Msg>(highlighted: bool, indent: usize, msg: &M) -> BoxedWidget {
    HJoin::new(vec![
        Segment::new(Padding::new(time::widget(Some(msg.time()), highlighted)).right(1)),
        Segment::new(Padding::new(Text::new(msg.nick())).right(1)),
        Segment::new(Text::new(msg.content()).wrap(true)),
    ])
    .into()
}

pub fn msg_placeholder(highlighted: bool, indent: usize) -> BoxedWidget {
    HJoin::new(vec![
        Segment::new(Padding::new(time::widget(None, highlighted)).right(1)),
        Segment::new(Text::new("[...]")),
    ])
    .into()
}
