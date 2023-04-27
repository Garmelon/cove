mod keys;

pub use cove_macro::KeyGroup;
use crossterm::event::KeyEvent;

pub use crate::keys::*;

/// A group of related key bindings.
pub trait KeyGroup {
    fn bindings(&self) -> Vec<(&KeyBinding, &'static str)>;
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
}

impl InputEvent {
    pub fn from_crossterm_event(event: crossterm::event::Event) -> Option<Self> {
        use crossterm::event::Event::*;
        match event {
            Key(event) => Some(Self::Key(event)),
            Paste(string) => Some(Self::Paste(string)),
            _ => None,
        }
    }

    pub fn key_event(&self) -> Option<KeyEvent> {
        match self {
            Self::Key(event) => Some(*event),
            _ => None,
        }
    }

    pub fn paste_event(&self) -> Option<&str> {
        match self {
            Self::Paste(string) => Some(string),
            _ => None,
        }
    }

    pub fn matches<S: ToString>(&self, binding: &KeyBinding) -> bool {
        match self.key_event() {
            Some(event) => binding.matches(event),
            None => false,
        }
    }
}
