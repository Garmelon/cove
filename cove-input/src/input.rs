use crossterm::event::KeyEvent;

use crate::KeyBinding;

pub enum Entry {
    Space,
    Category(String),
    Binding(KeyBinding, String),
}

pub struct Input {
    event: Option<KeyEvent>,

    record_entries: bool,
    entries: Vec<Entry>,
}

impl Input {
    pub fn new_from_event(event: KeyEvent) -> Self {
        Self {
            event: Some(event),
            record_entries: false,
            entries: vec![],
        }
    }

    pub fn new_recording() -> Self {
        Self {
            event: None,
            record_entries: true,
            entries: vec![],
        }
    }

    pub fn space<S: ToString>(&mut self) {
        if self.record_entries {
            self.entries.push(Entry::Space);
        }
    }

    pub fn category<S: ToString>(&mut self, name: S) {
        if self.record_entries {
            self.entries.push(Entry::Category(name.to_string()));
        }
    }

    pub fn matches<S: ToString>(&mut self, binding: &KeyBinding, description: S) -> bool {
        let matches = if let Some(event) = self.event {
            binding.matches(event)
        } else {
            false
        };

        if self.record_entries {
            self.entries
                .push(Entry::Binding(binding.clone(), description.to_string()));
        }

        matches
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}
