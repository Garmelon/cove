use std::convert::Infallible;
use std::env;

use jiff::tz::TimeZone;

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

/// Load a [`TimeZone`] specified by the `TZ` environment varible, or by the
/// provided string if the environment variable does not exist.
///
/// If a string is provided, it is interpreted in the same format that the `TZ`
/// environment variable uses.
///
/// If no `TZ` environment variable could be found and no string is provided,
/// the system local time (or UTC on Windows) is used.
pub fn load_time_zone(tz_string: Option<&str>) -> Result<TimeZone, jiff::Error> {
    let env_string = env::var("TZ").ok();
    let tz_string = env_string.as_ref().map(|s| s as &str).or(tz_string);

    let Some(tz_string) = tz_string else {
        return Ok(TimeZone::system());
    };

    if tz_string == "localtime" {
        return Ok(TimeZone::system());
    }

    if let Some(tz_string) = tz_string.strip_prefix(':') {
        return TimeZone::get(tz_string);
    }

    // The time zone is either a manually specified string or a file in the tz
    // database. We'll try to parse it as a manually specified string first
    // because that doesn't require a fs lookup.
    if let Ok(tz) = TimeZone::posix(tz_string) {
        return Ok(tz);
    }

    TimeZone::get(tz_string)
}

pub fn caesar(text: &str, by: i8) -> String {
    let by = by.rem_euclid(26) as u8;
    text.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                let c = c as u8 - b'a';
                let c = (c + by) % 26;
                (c + b'a') as char
            } else if c.is_ascii_uppercase() {
                let c = c as u8 - b'A';
                let c = (c + by) % 26;
                (c + b'A') as char
            } else {
                c
            }
        })
        .collect()
}
