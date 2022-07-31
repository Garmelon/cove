use crossterm::style::{ContentStyle, Stylize};
use time::format_description::FormatItem;
use time::macros::format_description;

use crate::euph::api::Time;
use crate::ui::widgets::background::Background;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

const TIME_FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day] [hour]:[minute]");
const TIME_EMPTY: &str = "                ";

fn style() -> ContentStyle {
    ContentStyle::default().grey()
}

fn style_inverted() -> ContentStyle {
    ContentStyle::default().black().on_white()
}

pub fn widget(time: Option<Time>, highlighted: bool) -> BoxedWidget {
    let text = if let Some(time) = time {
        time.0.format(TIME_FORMAT).expect("could not format time")
    } else {
        TIME_EMPTY.to_string()
    };

    let style = if highlighted {
        style_inverted()
    } else {
        style()
    };

    Background::new(Text::new((text, style)))
        .style(style)
        .into()
}
