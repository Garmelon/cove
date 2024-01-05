use std::convert::Infallible;
use std::env;

use time::{OffsetDateTime, UtcOffset};
use tz::{TimeZone, TzError};

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
pub fn load_time_zone(tz_string: Option<&str>) -> Result<TimeZone, TzError> {
    let env_string = env::var("TZ").ok();
    let tz_string = env_string.as_ref().map(|s| s as &str).or(tz_string);

    match &tz_string {
        // At the moment, TimeZone::from_posix_tz does not support "localtime"
        // on Windows, so we handle that case manually.
        Some("localtime") | None => TimeZone::local(),
        Some(tz_string) => TimeZone::from_posix_tz(tz_string),
    }
}

pub fn convert_to_time_zone(tz: &TimeZone, time: OffsetDateTime) -> Option<OffsetDateTime> {
    let utc_offset_in_seconds = tz
        .find_local_time_type(time.unix_timestamp())
        .ok()?
        .ut_offset();

    let utc_offset = UtcOffset::from_whole_seconds(utc_offset_in_seconds).ok()?;

    Some(time.to_offset(utc_offset))
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
