mod input;
mod keys;

pub use cove_macro::KeyGroup;

pub use input::*;
pub use keys::*;

/// A group of related key bindings.
pub trait KeyGroup {
    type Event;

    fn event(&self, input: &mut Input) -> Option<Self::Event>;
}
