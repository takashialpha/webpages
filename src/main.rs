// See lib.rs: deeply nested view-tree future types overflow the default depth in release.
#![recursion_limit = "256"]

/// Installs the process-wide `tracing` subscriber, formatted for the journal
/// that captures our stdout: color only when stdout is a terminal, and no
/// timestamp when journald owns stdout, since it sets `JOURNAL_STREAM` and
/// stamps every entry itself.
///
/// Fails only if a subscriber is already installed.
#[cfg(feature = "ssr")]
fn init_tracing() -> Result<(), tracing_subscriber::util::TryInitError> {
    use std::io::IsTerminal as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    use tracing_subscriber::{EnvFilter, fmt};

    // Without `RUST_LOG`: our own lifecycle lines, and warnings and errors from
    // everything else. Request logging sits a level below, at `webpages=debug`.
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

/// Routes panics through the log rather than raw stderr, so one arrives in the
/// journal at `ERROR` with the same shape as every other line.
///
/// The panicking request still loses its connection, which a reverse proxy in
/// front turns into a 502. Catching the unwind to answer with a 500 instead
/// would need a dependency, and would cover only panics that happen before the
/// response starts streaming, whereas this covers every one.
///
/// A backtrace rides along when `RUST_BACKTRACE` asks for one, matching what
/// the default hook would have printed.
#[cfg(feature = "ssr")]
fn init_panic_logging() {
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

/// Logs one line per request: method, path, status, and how long it took.
///
/// One line is the whole point. A span-based tracing layer splits the same
/// facts across a start event, a finish event, and an end-of-stream event,
/// which triples the volume of an access log for nothing.
#[cfg(feature = "ssr")]
async fn log_request(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Both are cheap handle clones, not copies of the underlying bytes.
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

/// Installs the termination handlers up front, so a failure to do so is a
/// startup error rather than a shutdown that never arrives. The returned future
/// resolves on the first signal.
///
/// systemd sends SIGTERM on stop and restart; SIGINT is Ctrl-C in a foreground
/// run.
#[cfg(feature = "ssr")]
fn shutdown_signal() -> std::io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};
    use tracing::info;

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;

    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => info!("received SIGINT, draining connections"),
            _ = terminate.recv() => info!("received SIGTERM, draining connections"),
        }
    })
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> std::process::ExitCode {
    use axum::Router;
    use axum::http::header;
    use axum::middleware::from_fn;
    use axum::routing::get;
    use leptos::config::get_configuration;
    use leptos_axum::render_app_to_stream;
    use std::process::ExitCode;
    use tokio::net::TcpListener;
    use tracing::{error, info, warn};
    use webpages::app::shell;

    // First, so every failure below has somewhere to go. An error here means a
    // subscriber is already installed, which is itself reportable.
    if let Err(error) = init_tracing() {
        warn!(%error, "keeping the tracing subscriber already installed");
    }
    init_panic_logging();

    // Leptos renders through a global spawner. `leptos_axum` installs one from
    // inside its router helpers; serving a single route by hand skips that, and
    // every render panics on the first spawn. An error means one is already set,
    // which is equally fine.
    let _ = any_spawner::Executor::init_tokio();

    let conf = match get_configuration(None) {
        Ok(conf) => conf,
        Err(error) => {
            error!(%error, "could not load the leptos configuration");
            return ExitCode::FAILURE;
        }
    };
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;

    // The quiet bad deploy: the binary comes up fine, but `LEPTOS_SITE_ROOT`
    // points nowhere and every page renders without CSS or WASM.
    if !std::path::Path::new(&*leptos_options.site_root).is_dir() {
        warn!(
            site_root = %leptos_options.site_root,
            "site root is not a directory, static assets will 404",
        );
    }

    // The other quiet one. Leptos looks for the hash file beside the binary,
    // not under the site root, and falls back to unhashed bundle names that
    // cargo-leptos never wrote, so the page arrives with no CSS and no
    // hydration. Mirror that lookup rather than guessing at the path.
    if leptos_options.hash_files {
        let beside_binary = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(&*leptos_options.hash_file)));
        if !beside_binary.is_some_and(|path| path.is_file()) {
            warn!(
                hash_file = %leptos_options.hash_file,
                "hash file is missing beside the binary, the bundle will not load",
            );
        }
    }

    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(error) => {
            error!(%error, "could not install the signal handlers");
            return ExitCode::FAILURE;
        }
    };

    // The site is a single page, so the sitemap is one fixed URL. Built from
    // `SITE_URL` rather than spelled out again, and leaked because it is read on
    // every hit and never changes.
    let sitemap: &'static str = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{}</loc></url></urlset>"#,
        webpages::SITE_URL
    )
    .leak();

    // Process start, for `/health`. `Instant` is monotonic, so this survives a
    // wall-clock adjustment.
    let started = std::time::Instant::now();

    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            error!(address = %addr, %error, "could not bind the listener");
            return ExitCode::FAILURE;
        }
    };
    info!(
        // The socket's own address, which differs from the configured one
        // whenever that asks for port 0 or an unspecified host.
        address = %listener.local_addr().unwrap_or(addr),
        build = webpages::BUILD,
        site_root = %leptos_options.site_root,
        site_pkg_dir = %leptos_options.site_pkg_dir,
        "listening",
    );

    let app = Router::new()
        .route(
            "/sitemap.xml",
            get(move || async move {
                (
                    [
                        (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
                        (header::CACHE_CONTROL, "public, max-age=3600"),
                    ],
                    sitemap,
                )
            }),
        )
        .route(
            "/health",
            get(move || async move {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    format!(
                        r#"{{"status":"ok","uptime_seconds":{},"build":"{}"}}"#,
                        started.elapsed().as_secs(),
                        webpages::BUILD
                    ),
                )
            }),
        )
        .route(
            "/",
            get(render_app_to_stream({
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            })),
        )
        // Serves everything under the site root, and answers anything else with
        // a 404. The pages the old site had are simply gone, so they land here.
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(from_fn(log_request))
        .with_state(leptos_options);

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
    {
        error!(%error, "server stopped");
        return ExitCode::FAILURE;
    }

    info!("shutdown complete");
    ExitCode::SUCCESS
}

#[cfg(not(feature = "ssr"))]
pub const fn main() {}
