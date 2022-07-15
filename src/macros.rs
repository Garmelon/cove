macro_rules! some_or_return {
    ($e:expr) => {
        match $e {
            None => return,
            Some(result) => result,
        }
    };
    ($e:expr, $ret:expr) => {
        match $e {
            None => return $ret,
            Some(result) => result,
        }
    };
}
pub(crate) use some_or_return;
