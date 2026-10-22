//! UTC calendar helpers without external crates (log timestamps, daily stats).

use std::time::{SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's civil-from-days.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Today (UTC) as days since the epoch.
pub fn today() -> i64 {
    (now_secs() / 86_400) as i64
}

/// `YYYY-MM-DD` for a day number; sorts chronologically as text.
pub fn date_string(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD HH:MM:SS` (UTC) for log lines.
pub fn timestamp() -> String {
    let secs = now_secs();
    let rem = secs % 86_400;
    format!("{} {:02}:{:02}:{:02}", date_string((secs / 86_400) as i64), rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `YYYY-MM-DD HH:MM:SS UTC` for a given time.
pub fn timestamp_at(secs: u64) -> String {
    let rem = secs % 86_400;
    format!("{} {:02}:{:02}:{:02} UTC", date_string((secs / 86_400) as i64), rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `YYYY-MM-DDTHHMMSSZ` (UTC) for file names: sorts chronologically, no colons.
pub fn file_stamp(secs: u64) -> String {
    let rem = secs % 86_400;
    format!("{}T{:02}{:02}{:02}Z", date_string((secs / 86_400) as i64), rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(date_string(0), "1970-01-01");
        assert_eq!(date_string(19_723), "2024-01-01");
        assert_eq!(date_string(19_782), "2024-02-29");
        assert_eq!(date_string(-1), "1969-12-31");
        assert_eq!(timestamp().len(), 19);
    }
}
