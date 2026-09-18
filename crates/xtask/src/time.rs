//! Minimal UTC wall-clock formatting, without pulling in a date-time crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as an ISO 8601 UTC timestamp, e.g.
/// `2026-09-18T12:34:56Z`.
pub(crate) fn iso8601_now() -> String {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX);
    let secs_of_day = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Converts a day count since the Unix epoch (1970-01-01) into a proleptic
/// Gregorian `(year, month, day)` triple.
///
/// Port of Howard Hinnant's `civil_from_days` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html>).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = u64::try_from(z - era * 146_097).unwrap_or(0); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(0); // [1, 31]
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(0); // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn epoch_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn known_dates_round_trip() {
        // 2026-09-18 is 20,714 days after the Unix epoch.
        assert_eq!(civil_from_days(20_714), (2026, 9, 18));
        // 2000-03-01, just after a leap day.
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        // 2000-02-29, the leap day itself.
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
