//! The units the app counts time in. Charts place minutes along an axis as
//! floats, hence the float twins.

pub const SECS_PER_MINUTE: u64 = 60;
pub const MINUTES_PER_HOUR: u64 = 60;
pub const HOURS_PER_DAY: u64 = 24;
pub const SECS_PER_HOUR: u64 = SECS_PER_MINUTE * MINUTES_PER_HOUR;
pub const SECS_PER_DAY: u64 = SECS_PER_HOUR * HOURS_PER_DAY;
pub const SECS_PER_MINUTE_F: f32 = 60.;
pub const MINUTES_PER_HOUR_F: f32 = 60.;

/// Minutes since local midnight, now (at least one, so a span from midnight
/// is never empty).
pub fn minutes_today() -> f32 {
    let now = jiff::Zoned::now();
    (now.hour() as f32 * MINUTES_PER_HOUR_F
        + now.minute() as f32
        + now.second() as f32 / SECS_PER_MINUTE_F)
        .max(1.)
}

/// This minute, counted from the epoch, as craked counts traffic.
pub fn epoch_minute() -> u64 {
    jiff::Timestamp::now().as_second() as u64 / SECS_PER_MINUTE
}

/// Local midnight, as an epoch minute.
pub fn midnight_minute() -> u64 {
    jiff::Zoned::now()
        .start_of_day()
        .map(|z| z.timestamp().as_second() as u64 / SECS_PER_MINUTE)
        .unwrap_or(0)
}
