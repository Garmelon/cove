mod input;
mod keys;

pub use input::*;
pub use keys::*;

pub trait Group {
    type Action;

    fn action(&self, input: &mut Input) -> Option<Self::Action>;
}
