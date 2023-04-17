use std::convert::Infallible;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::style::Stylize;
use toss::widgets::{Empty, Join2, Text};
use toss::{Style, Styled, Widget, WidgetExt};

use super::widgets::{ListBuilder, ListState};
use super::UiError;

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
}

impl InputEvent {
    pub fn from_event(event: Event) -> Option<Self> {
        match event {
            crossterm::event::Event::Key(key) => Some(Self::Key(key.into())),
            crossterm::event::Event::Paste(text) => Some(Self::Paste(text)),
            _ => None,
        }
    }
}

/// A key event data type that is a bit easier to pattern match on than
/// [`crossterm::event::KeyEvent`].
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: event.code,
            shift: event.modifiers.contains(KeyModifiers::SHIFT),
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
        }
    }
}

#[rustfmt::skip]
#[allow(unused_macro_rules)]
macro_rules! key {
    // key!(Paste text)
    ( Paste $text:ident ) => { crate::ui::input::InputEvent::Paste($text) };

    // key!('a')
    (        $key:literal ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: false, alt: false, }) };
    ( Ctrl + $key:literal ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: true,  alt: false, }) };
    (  Alt + $key:literal ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: false, alt: true,  }) };

    // key!(Char c)
    (        Char $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: false, alt: false, }) };
    ( Ctrl + Char $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: true,  alt: false, }) };
    (  Alt + Char $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::Char($key), shift: _, ctrl: false, alt: true,  }) };

    // key!(F n)
    (         F $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::F($key), shift: false, ctrl: false, alt: false, }) };
    ( Shift + F $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::F($key), shift: true,  ctrl: false, alt: false, }) };
    (  Ctrl + F $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::F($key), shift: false, ctrl: true,  alt: false, }) };
    (   Alt + F $key:pat ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::F($key), shift: false, ctrl: false, alt: true,  }) };

    // key!(other)
    (         $key:ident ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::$key, shift: false, ctrl: false, alt: false, }) };
    ( Shift + $key:ident ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::$key, shift: true,  ctrl: false, alt: false, }) };
    (  Ctrl + $key:ident ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::$key, shift: false, ctrl: true,  alt: false, }) };
    (   Alt + $key:ident ) => { crate::ui::input::InputEvent::Key(crate::ui::input::KeyEvent { code: crossterm::event::KeyCode::$key, shift: false, ctrl: false, alt: true,  }) };
}
pub(crate) use key;

enum Row {
    Empty,
    Heading(String),
    Binding(String, String),
    BindingContd(String),
}

pub struct KeyBindingsList(Vec<Row>);

impl KeyBindingsList {
    /// Width of the left column of key bindings.
    const BINDING_WIDTH: u16 = 20;

    pub fn new() -> Self {
        Self(vec![])
    }

    fn binding_style() -> Style {
        Style::new().cyan()
    }

    fn row_widget(row: Row) -> impl Widget<UiError> {
        match row {
            Row::Empty => Text::new("").first3(),

            Row::Heading(name) => Text::new((name, Style::new().bold())).first3(),

            Row::Binding(binding, description) => Join2::horizontal(
                Text::new((binding, Self::binding_style()))
                    .padding()
                    .with_right(1)
                    .resize()
                    .with_min_width(Self::BINDING_WIDTH)
                    .segment(),
                Text::new(description).segment(),
            )
            .second3(),

            Row::BindingContd(description) => Join2::horizontal(
                Empty::new().with_width(Self::BINDING_WIDTH).segment(),
                Text::new(description).segment(),
            )
            .third3(),
        }
    }

    pub fn widget(self, list_state: &mut ListState<Infallible>) -> impl Widget<UiError> + '_ {
        let binding_style = Self::binding_style();

        let hint_text = Styled::new("jk/↓↑", binding_style)
            .then_plain(" to scroll, ")
            .then("esc", binding_style)
            .then_plain(" to close");

        let hint = Text::new(hint_text)
            .padding()
            .with_horizontal(1)
            .float()
            .with_horizontal(0.5)
            .with_vertical(0.0);

        let mut list_builder = ListBuilder::new();
        for row in self.0 {
            list_builder.add_unsel(Self::row_widget(row));
        }

        list_builder
            .build(list_state)
            .padding()
            .with_horizontal(1)
            .border()
            .below(hint)
            .background()
            .float()
            .with_center()
    }

    pub fn empty(&mut self) {
        self.0.push(Row::Empty);
    }

    pub fn heading(&mut self, name: &str) {
        self.0.push(Row::Heading(name.to_string()));
    }

    pub fn binding(&mut self, binding: &str, description: &str) {
        self.0
            .push(Row::Binding(binding.to_string(), description.to_string()));
    }

    pub fn binding_ctd(&mut self, description: &str) {
        self.0.push(Row::BindingContd(description.to_string()));
    }
}
