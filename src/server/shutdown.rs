//! Stopping without cutting a page off mid render.

use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

/// Installs the termination handlers up front, so failing to is a startup
/// error rather than a shutdown that never comes. The future resolves on the
/// first signal.
///
/// systemd sends SIGTERM on stop and restart; SIGINT is Ctrl-C.
///
/// # Errors
///
/// Whatever stopped the handlers being installed.
pub fn on_signal() -> std::io::Result<impl Future<Output = ()>> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;

    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => info!("received SIGINT, draining connections"),
            _ = terminate.recv() => info!("received SIGTERM, draining connections"),
        }
    })
}
