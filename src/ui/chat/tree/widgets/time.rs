use crossterm::style::ContentStyle;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;
use time::UtcOffset;

use crate::config::Config;
use crate::ui::widgets::background::Background;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

const TIME_FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day] [hour]:[minute]");
const TIME_WIDTH: u16 = 16;

pub fn widget(
    time: Option<OffsetDateTime>,
    style: ContentStyle,
    config: Option<&Config>,
) -> BoxedWidget {
    let time_offset = if let Some(config) = config {
        config.timezone_offset()
    } else {
        UtcOffset::UTC
    };

    if let Some(time) = time {
        let time = time.to_offset(time_offset);
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
