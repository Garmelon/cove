mod event;
mod keys;

pub use cove_macro::KeyGroup;

pub use event::*;
pub use keys::*;

/// A group of related key bindings.
pub trait KeyGroup {
    fn bindings(&self) -> Vec<(&KeyBinding, &'static str)>;
}
