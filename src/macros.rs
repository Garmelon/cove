macro_rules! some_or_return {
    ($e:expr) => {
        match $e {
            Some(result) => result,
            None => return,
        }
    };
    ($e:expr, $ret:expr) => {
        match $e {
            Some(result) => result,
            None => return $ret,
        }
    };
}
pub(crate) use some_or_return;

macro_rules! ok_or_return {
    ($e:expr) => {
        match $e {
            Ok(result) => result,
            Err(_) => return,
        }
    };
    ($e:expr, $ret:expr) => {
        match $e {
            Ok(result) => result,
            Err(_) => return $ret,
        }
    };
}
pub(crate) use ok_or_return;

// TODO Get rid of this macro as much as possible
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
