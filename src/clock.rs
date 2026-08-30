//! Time, taken from the server rather than the browser.
//!
//! A terminal pretending to be a server should answer with the server's clock,
//! not one that might be skewed, wrong by years, or faked. So the server stamps
//! the document with its UTC time and its uptime, and the browser only measures
//! how long the page has been open. The server stays the authority, and the
//! clock still moves while you sit there.

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
/// Pinned by the first call, which the server makes at startup, so this is the
/// process's life and not the machine's. `Instant` only counts up, so putting
/// the wall clock back cannot move it.
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
/// `performance.now()` only counts up and keeps running in a background tab,
/// unlike an interval, which browsers throttle and which would undercount.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn since_load_millis() -> f64 {
    leptos::prelude::window()
        .performance()
        .map_or(0.0, |performance| performance.now())
}

/// A number the server stamped onto the document.
#[cfg(feature = "hydrate")]
fn stamped(attribute: &str) -> i64 {
    leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute(attribute))
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

/// The server's clock, plus however long the page has been open.
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
    // Const-evaluated by the macro, so nothing parses this at runtime.
    let shape = time::macros::format_description!(
        "[weekday repr:short] [month repr:short] [day padding:space] \
         [hour]:[minute]:[second] UTC [year]"
    );

    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .format(shape)
        .unwrap_or_default()
}

/// The server has no page to have been open and never runs a frame.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub const fn since_load_millis() -> f64 {
    0.0
}
