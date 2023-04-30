mod keys;

use std::io;
use std::sync::Arc;

pub use cove_macro::KeyGroup;
use crossterm::event::{Event, KeyEvent};
use parking_lot::FairMutex;
use toss::{Frame, Terminal, WidthDb};

pub use crate::keys::*;

pub struct KeyBindingInfo<'a> {
    pub name: &'static str,
    pub binding: &'a KeyBinding,
    pub description: &'static str,
}

/// A group of related key bindings.
pub trait KeyGroup {
    const DESCRIPTION: &'static str;

    fn bindings(&self) -> Vec<KeyBindingInfo<'_>>;
}

pub struct KeyGroupInfo<'a> {
    pub name: &'static str,
    pub description: &'static str,
    pub bindings: Vec<KeyBindingInfo<'a>>,
}

impl<'a> KeyGroupInfo<'a> {
    pub fn new<G: KeyGroup>(name: &'static str, group: &'a G) -> Self {
        Self {
            name,
            description: G::DESCRIPTION,
            bindings: group.bindings(),
        }
    }
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

    pub fn frame(&mut self) -> &mut Frame {
        self.terminal.frame()
    }

    pub fn widthdb(&mut self) -> &mut WidthDb {
        self.terminal.widthdb()
    }

    pub fn prompt(&mut self, initial_text: &str) -> io::Result<String> {
        let guard = self.crossterm_lock.lock();
        self.terminal.suspend().expect("failed to suspend");
        let content = edit::edit(initial_text);
        self.terminal.unsuspend().expect("fauled to unsuspend");
        drop(guard);

        content
    }
}
