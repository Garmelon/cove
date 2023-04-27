use crossterm::event::KeyEvent;

use crate::KeyBinding;

enum Mode {
    Record,
    Key(KeyEvent),
    Paste(String),
}

impl Mode {
    fn from_crossterm_event(event: crossterm::event::Event) -> Option<Self> {
        use crossterm::event::Event::*;
        match event {
            Key(event) => Some(Self::Key(event)),
            Paste(string) => Some(Self::Paste(string)),
            _ => None,
        }
    }
}

pub enum Entry {
    Space,
    Category(String),
    Binding(KeyBinding, String),
}

pub struct InputEvent {
    mode: Mode,
    entries: Vec<Entry>,
}

impl InputEvent {
    pub fn new_recording() -> Self {
        Self {
            mode: Mode::Record,
            entries: vec![],
        }
    }

    pub fn from_crossterm_event(event: crossterm::event::Event) -> Option<Self> {
        Some(Self {
            mode: Mode::from_crossterm_event(event)?,
            entries: vec![],
        })
    }

    fn recording(&self) -> bool {
        matches!(self.mode, Mode::Record)
    }

    pub fn space<S: ToString>(&mut self) {
        if self.recording() {
            self.entries.push(Entry::Space);
        }
    }

    pub fn category<S: ToString>(&mut self, name: S) {
        if self.recording() {
            self.entries.push(Entry::Category(name.to_string()));
        }
    }

    pub fn key_event(&self) -> Option<KeyEvent> {
        if let Mode::Key(event) = &self.mode {
            Some(*event)
        } else {
            None
        }
    }

    pub fn paste_event(&self) -> Option<&str> {
        if let Mode::Paste(string) = &self.mode {
            Some(string)
        } else {
            None
        }
    }

    pub fn matches_key_binding<S: ToString>(
        &mut self,
        binding: &KeyBinding,
        description: S,
    ) -> bool {
        if self.recording() {
            self.entries
                .push(Entry::Binding(binding.clone(), description.to_string()));
        }

        if let Some(event) = self.key_event() {
            binding.matches(event)
        } else {
            false
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}
