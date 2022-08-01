use crossterm::style::{ContentStyle, Stylize};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::ui::widgets::background::Background;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

const TIME_FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day] [hour]:[minute]");
const TIME_WIDTH: u16 = 16;

fn style() -> ContentStyle {
    ContentStyle::default().grey()
}

fn style_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub fn widget(time: Option<OffsetDateTime>, highlighted: bool) -> BoxedWidget {
    let style = if highlighted {
        style_inverted()
    } else {
        style()
    };

    if let Some(time) = time {
        let text = time.format(TIME_FORMAT).expect("could not format time");
        Background::new(Text::new((text, style)))
            .style(style)
            .into()
    } else {
        Background::new(Empty::new().width(TIME_WIDTH))
            .style(style)
            .into()
    }
}
