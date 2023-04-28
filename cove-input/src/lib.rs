mod keys;

use std::sync::Arc;

pub use cove_macro::KeyGroup;
use crossterm::event::{Event, KeyEvent};
use parking_lot::FairMutex;
use toss::Terminal;

pub use crate::keys::*;

/// A group of related key bindings.
pub trait KeyGroup {
    fn bindings(&self) -> Vec<(&KeyBinding, &'static str)>;
}

pub struct InputEvent<'a> {
    event: crossterm::event::Event,
    terminal: &'a mut Terminal,
    crossterm_lock: Arc<FairMutex<()>>,
}

impl<'a> InputEvent<'a> {
    pub fn new(
        event: Event,
        terminal: &'a mut Terminal,
        crossterm_lock: Arc<FairMutex<()>>,
    ) -> Self {
        Self {
            event,
            terminal,
            crossterm_lock,
        }
    }

    pub fn key_event(&self) -> Option<KeyEvent> {
        match &self.event {
            Event::Key(event) => Some(*event),
            _ => None,
        }
    }

    pub fn paste_event(&self) -> Option<&str> {
        match &self.event {
            Event::Paste(string) => Some(string),
            _ => None,
        }
    }

    pub fn matches(&self, binding: &KeyBinding) -> bool {
        match self.key_event() {
            Some(event) => binding.matches(event),
            None => false,
        }
    }
}
