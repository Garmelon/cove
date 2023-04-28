//! A scrollable popup showing the current key bindings.

use std::convert::Infallible;

use cove_config::{Config, Keys};
use cove_input::{InputEvent, KeyBinding, KeyGroup};
use crossterm::style::Stylize;
use toss::widgets::{Either2, Join2, Padding, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use super::widgets::{ListBuilder, ListState, Popup};
use super::{util, UiError};

type Line = Either2<Text, Join2<Padding<Text>, Text>>;
type Builder = ListBuilder<'static, Infallible, Line>;

fn render_empty(builder: &mut Builder) {
    builder.add_unsel(Text::new("").first2());
}

fn render_title(builder: &mut Builder, title: &str) {
    let style = Style::new().bold().magenta();
    builder.add_unsel(Text::new(Styled::new(title, style)).first2());
}

fn render_binding(builder: &mut Builder, binding: &KeyBinding, description: &str) {
    let style = Style::new().cyan();
    let mut keys = Styled::default();
    for key in binding.keys() {
        if !keys.text().is_empty() {
            keys = keys.then_plain(", ");
        }
        keys = keys.then(key.to_string(), style);
    }

    builder.add_unsel(
        Join2::horizontal(
            Text::new(description)
                .with_wrap(false)
                .padding()
                .with_right(2)
                .with_stretch(true)
                .segment(),
            Text::new(keys).with_wrap(false).segment().with_fixed(true),
        )
        .second2(),
    )
}

fn render_group<G: KeyGroup>(builder: &mut Builder, group: &G) {
    for (binding, description) in group.bindings() {
        render_binding(builder, binding, description);
    }
}

pub fn widget<'a>(
    list: &'a mut ListState<Infallible>,
    config: &Config,
) -> impl Widget<UiError> + 'a {
    let mut list_builder = ListBuilder::new();

    render_title(&mut list_builder, "General");
    render_group(&mut list_builder, &config.keys.general);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Scrolling");
    render_group(&mut list_builder, &config.keys.scroll);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Cursor movement");
    render_group(&mut list_builder, &config.keys.cursor);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Editor cursor movement");
    render_group(&mut list_builder, &config.keys.editor.cursor);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Editor actions");
    render_group(&mut list_builder, &config.keys.editor.action);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Room list actions");
    render_group(&mut list_builder, &config.keys.rooms.action);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Tree cursor movement");
    render_group(&mut list_builder, &config.keys.tree.cursor);
    render_empty(&mut list_builder);
    render_title(&mut list_builder, "Tree actions");
    render_group(&mut list_builder, &config.keys.tree.action);

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
