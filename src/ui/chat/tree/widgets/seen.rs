use crossterm::style::Stylize;
use toss::Style;

use crate::ui::widgets::background::Background;
use crate::ui::widgets::empty::Empty;
use crate::ui::widgets::text::Text;
use crate::ui::widgets::BoxedWidget;

const UNSEEN: &str = "*";
const WIDTH: u16 = 1;

fn seen_style() -> Style {
    Style::new().black().on_green()
}

pub fn widget(seen: bool) -> BoxedWidget {
    if seen {
        Empty::new().width(WIDTH).into()
    } else {
        let style = seen_style();
        Background::new(Text::new((UNSEEN, style)))
            .style(style)
            .into()
    }
}
