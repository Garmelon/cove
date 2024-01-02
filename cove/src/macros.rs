macro_rules! logging_unwrap {
    ($e:expr) => {
        match $e {
            Ok(value) => value,
            Err(err) => {
                log::error!("{err}");
                panic!("{err}");
            }
        }
    };
}
pub(crate) use logging_unwrap;
