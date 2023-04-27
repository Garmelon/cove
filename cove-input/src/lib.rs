mod event;
mod keys;

pub use cove_macro::KeyGroup;

pub use event::*;
pub use keys::*;

/// A group of related key bindings.
pub trait KeyGroup {
    type Event;

    fn match_input_event(&self, event: &mut InputEvent) -> Option<Self::Event>;
}
