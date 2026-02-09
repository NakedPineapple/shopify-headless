//! Database operations for the support system.
//!
//! All queries target `storefront.support_*` tables in the `np_storefront` database.

pub mod conversation;
pub mod knowledge;
pub mod message;
pub mod ticket;

use chrono::{DateTime, Datelike, Timelike, Utc};

/// Convert chrono `DateTime<Utc>` to `time::OffsetDateTime` for `SQLx` bind params.
///
/// When both `chrono` and `time` features are active for sqlx, the offline
/// macro expansion resolves `TIMESTAMPTZ` to `time::OffsetDateTime`.
///
/// # Panics
///
/// Panics if the chrono `DateTime` contains values outside the `time` crate's
/// valid range (month, day, hour, minute, second). This is unreachable for
/// valid `DateTime<Utc>` values.
#[must_use]
pub fn to_time_offset(dt: DateTime<Utc>) -> time::OffsetDateTime {
    let date = time::Date::from_calendar_date(
        dt.year(),
        time::Month::try_from(u8::try_from(dt.month()).expect("month in range"))
            .expect("valid month"),
        u8::try_from(dt.day()).expect("day in range"),
    )
    .expect("valid date");
    let t = time::Time::from_hms_nano(
        u8::try_from(dt.hour()).expect("hour in range"),
        u8::try_from(dt.minute()).expect("minute in range"),
        u8::try_from(dt.second()).expect("second in range"),
        dt.timestamp_subsec_nanos(),
    )
    .expect("valid time");
    time::OffsetDateTime::new_utc(date, t)
}
