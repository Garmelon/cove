//! A scrollable popup showing the current key bindings.

use std::convert::Infallible;

use cove_config::{Config, Keys};
use cove_input::{InputEvent, KeyBinding, KeyBindingInfo, KeyGroupInfo};
use crossterm::style::Stylize;
use toss::widgets::{Either2, Join2, Padding, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use super::widgets::{ListBuilder, ListState, Popup};
use super::{util, UiError};

type Line = Either2<Text, Join2<Padding<Text>, Text>>;
type Builder = ListBuilder<'static, Infallible, Line>;

pub fn format_binding(binding: &KeyBinding) -> Styled {
    let style = Style::new().cyan();
    let mut keys = Styled::default();

    for key in binding.keys() {
        if !keys.text().is_empty() {
            keys = keys.then_plain(", ");
        }
        keys = keys.then(key.to_string(), style);
    }

    if keys.text().is_empty() {
        keys = keys.then("unbound", style);
    }

    keys
}

fn render_empty(builder: &mut Builder) {
    builder.add_unsel(Text::new("").first2());
}

fn render_title(builder: &mut Builder, title: &str) {
    let style = Style::new().bold().magenta();
    builder.add_unsel(Text::new(Styled::new(title, style)).first2());
}

fn render_binding_info(builder: &mut Builder, binding_info: KeyBindingInfo<'_>) {
    builder.add_unsel(
        Join2::horizontal(
            Text::new(binding_info.description)
                .with_wrap(false)
                .padding()
                .with_right(2)
                .with_stretch(true)
                .segment(),
            Text::new(format_binding(binding_info.binding))
                .with_wrap(false)
                .segment()
                .with_fixed(true),
        )
        .second2(),
    )
}

fn render_group_info(builder: &mut Builder, group_info: KeyGroupInfo<'_>) {
    render_title(builder, group_info.description);
    for binding_info in group_info.bindings {
        render_binding_info(builder, binding_info);
    }
}

pub fn widget<'a>(
    list: &'a mut ListState<Infallible>,
    config: &Config,
) -> impl Widget<UiError> + 'a {
    let mut list_builder = ListBuilder::new();

    for group_info in config.keys.groups() {
        if !list_builder.is_empty() {
            render_empty(&mut list_builder);
        }
        render_group_info(&mut list_builder, group_info);
    }

    Popup::new(list_builder.build(list), "Key bindings")
}

pub fn handle_input_event(
    list: &mut ListState<Infallible>,
    event: &mut InputEvent<'_>,
    keys: &Keys,
) -> bool {
    // To make scrolling with the mouse wheel work as expected
    if event.matches(&keys.cursor.up) {
        list.scroll_up(1);
        return true;
    }
    if event.matches(&keys.cursor.down) {
        list.scroll_down(1);
        return true;
    }

    // List movement must come later, or it shadows the cursor movement keys
    if util::handle_list_input_event(list, event, keys) {
        return true;
    }

    false
}
