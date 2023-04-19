use std::convert::Infallible;

pub trait InfallibleExt {
    type Inner;

    fn infallible(self) -> Self::Inner;
}

impl<T> InfallibleExt for Result<T, Infallible> {
    type Inner = T;

    fn infallible(self) -> T {
        self.expect("infallible")
    }
}
