use std::convert::Infallible;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::style::Stylize;
use toss::{Style, Styled};

use super::widgets::background::Background;
use super::widgets::border::Border;
use super::widgets::empty::Empty;
use super::widgets::float::Float;
use super::widgets::join::{HJoin, Segment};
use super::widgets::layer::Layer;
use super::widgets::list::{List, ListState};
use super::widgets::padding::Padding;
use super::widgets::resize::Resize;
use super::widgets::text::Text;
use super::widgets::BoxedWidget;

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

/// Helper wrapper around a list widget for a more consistent key binding style.
pub struct KeyBindingsList(List<Infallible>);

impl KeyBindingsList {
    /// Width of the left column of key bindings.
    const BINDING_WIDTH: u16 = 20;

    pub fn new(state: &ListState<Infallible>) -> Self {
        Self(state.widget())
    }

    fn binding_style() -> Style {
        Style::new().cyan()
    }

    pub fn widget(self) -> BoxedWidget {
        let binding_style = Self::binding_style();
        Float::new(Layer::new(vec![
            Border::new(Background::new(Padding::new(self.0).horizontal(1))).into(),
            Float::new(
                Padding::new(Text::new(
                    Styled::new("jk/↓↑", binding_style)
                        .then_plain(" to scroll, ")
                        .then("esc", binding_style)
                        .then_plain(" to close"),
                ))
                .horizontal(1),
            )
            .horizontal(0.5)
            .into(),
        ]))
        .horizontal(0.5)
        .vertical(0.5)
        .into()
    }

    pub fn empty(&mut self) {
        self.0.add_unsel(Empty::new());
    }

    pub fn heading(&mut self, name: &str) {
        self.0.add_unsel(Text::new((name, Style::new().bold())));
    }

    pub fn binding(&mut self, binding: &str, description: &str) {
        let widget = HJoin::new(vec![
            Segment::new(
                Resize::new(Padding::new(Text::new((binding, Self::binding_style()))).right(1))
                    .min_width(Self::BINDING_WIDTH),
            ),
            Segment::new(Text::new(description)),
        ]);
        self.0.add_unsel(widget);
    }

    pub fn binding_ctd(&mut self, description: &str) {
        let widget = HJoin::new(vec![
            Segment::new(Resize::new(Empty::new()).min_width(Self::BINDING_WIDTH)),
            Segment::new(Text::new(description)),
        ]);
        self.0.add_unsel(widget);
    }
}
