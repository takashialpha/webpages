//! Time, taken from the server rather than the browser.
//!
//! A terminal pretending to be a server should answer with the server's clock.
//! The visitor's may be skewed, wrong by years, or deliberately faked, and none
//! of that should show up in `date`.
//!
//! So the server stamps the document with its own UTC time and its uptime, and
//! the browser only measures how long the page has been open. That keeps the
//! authority server-side while still advancing while you sit there.

/// Milliseconds since the unix epoch, UTC.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0))
}

/// Seconds this process has been up.
///
/// Pinned by the first call, which the server makes at startup, so the value is
/// the process lifetime rather than the machine's. `Instant` is monotonic, so a
/// wall-clock adjustment cannot move it.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn uptime_secs() -> i64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    i64::try_from(START.get_or_init(Instant::now).elapsed().as_secs()).unwrap_or(i64::MAX)
}

/// Milliseconds since this page was rendered.
///
/// `performance.now()` is monotonic and keeps running in a background tab,
/// unlike an interval, which browsers throttle and which would undercount.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn since_load_millis() -> f64 {
    leptos::prelude::window()
        .performance()
        .map_or(0.0, |performance| performance.now())
}

/// Reads back a number the server stamped onto the document.
#[cfg(feature = "hydrate")]
fn stamped(attribute: &str) -> i64 {
    leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute(attribute))
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

/// The server's clock, carried forward by however long the page has been open.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn now_millis() -> i64 {
    stamped("data-time") + elapsed_millis()
}

/// The server's uptime, carried forward the same way.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn uptime_secs() -> i64 {
    stamped("data-uptime") + elapsed_millis() / 1000
}

#[cfg(feature = "hydrate")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "a page open for 10^9 years is not a case worth handling"
)]
fn elapsed_millis() -> i64 {
    since_load_millis() as i64
}

/// `Tue Aug 25 21:33:24 UTC 2026`, the way `date -u` prints it.
#[must_use]
pub fn format_utc(millis: i64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let seconds = millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    // 1970-01-01 was a Thursday, so shifting by 4 puts Sunday at zero.
    let weekday = DAYS[usize::try_from((days + 4).rem_euclid(7)).unwrap_or(0)];
    let month_name = MONTHS[usize::try_from(month - 1).unwrap_or(0).min(11)];

    format!(
        "{weekday} {month_name} {day:2} {:02}:{:02}:{:02} UTC {year}",
        time / 3600,
        (time % 3600) / 60,
        time % 60,
    )
}

/// Days since the unix epoch to a civil date, by Howard Hinnant's algorithm.
/// Shifts the era to start in March so the leap day lands at the end of a year
/// and the month lengths follow a regular pattern.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// The server has no page to have been open, and never runs a frame.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub const fn since_load_millis() -> f64 {
    0.0
}
