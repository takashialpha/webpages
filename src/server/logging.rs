//! Where everything the server has to say goes: startup and shutdown lines,
//! panics, and one line per request.

/// Sets up logging, shaped for wherever stdout goes: colour only for a
/// terminal, and no timestamp under journald, which stamps every line itself
/// and sets `JOURNAL_STREAM` to say so.
///
/// # Errors
///
/// Only if a subscriber is already installed.
pub fn init_tracing() -> Result<(), tracing_subscriber::util::TryInitError> {
    use std::io::IsTerminal as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    use tracing_subscriber::{EnvFilter, fmt};

    // Without `RUST_LOG`: our startup and shutdown lines, and warnings from
    // everything else. Per-request lines sit below that, at `webpages=debug`.
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,webpages=info"));
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_terminal());

    if std::env::var_os("JOURNAL_STREAM").is_some() {
        subscriber.without_time().finish().try_init()
    } else {
        subscriber.finish().try_init()
    }
}

/// Sends panics to the log instead of raw stderr, so one lands in the journal
/// at `ERROR` looking like every other line, with a backtrace when
/// `RUST_BACKTRACE` asks for one.
///
/// The request still loses its connection and a proxy in front turns that into
/// a 502. Answering 500 instead would mean catching the unwind, which only
/// works before the response starts streaming; this covers every panic.
pub fn init_panic_logging() {
    use std::backtrace::{Backtrace, BacktraceStatus};

    std::panic::set_hook(Box::new(|info| {
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            tracing::error!(%backtrace, "{info}");
        } else {
            tracing::error!("{info}");
        }
    }));
}

/// One line per request: method, path, status, and how long it took.
///
/// One line is the point. A span-based layer spreads the same facts across a
/// start, a finish and an end-of-stream event, which is three times the access
/// log for nothing.
pub async fn log_request(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Handle clones, not copies of the bytes.
    let method = request.method().clone();
    let uri = request.uri().clone();

    let started = std::time::Instant::now();
    let response = next.run(request).await;

    tracing::debug!(
        %method,
        path = %uri.path(),
        status = response.status().as_u16(),
        elapsed = ?started.elapsed(),
        "request",
    );
    response
}
