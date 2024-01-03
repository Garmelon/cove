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

/// Load a [`TimeZone`] specified by a string, or by the `TZ` environment
/// variable if no string is provided.
///
/// If a string is provided, it is interpreted in the same format that the `TZ`
/// environment variable uses.
///
/// If no string and no `TZ` environment variable could be found, the system
/// local time is used.
pub fn load_time_zone(tz_string: Option<&str>) -> Result<TimeZone, TzError> {
    let env_tz = env::var("TZ").ok();
    let tz_string = tz_string.or(env_tz.as_ref().map(|s| s as &str));

    match &tz_string {
        // At the moment, TimeZone::from_posix_tz does not support "localtime" on windows, but other time zon
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
