// See lib.rs: nested view-tree future types overflow the default depth.
#![recursion_limit = "256"]
// Nothing in the server needs unsafe, so nothing may bring it back.
#![forbid(unsafe_code)]

/// Sets up logging, shaped for wherever stdout goes: colour only for a
/// terminal, and no timestamp under journald, which stamps every line itself
/// and sets `JOURNAL_STREAM` to say so.
///
/// # Errors
///
/// Only if a subscriber is already installed.
#[cfg(feature = "ssr")]
fn init_tracing() -> Result<(), tracing_subscriber::util::TryInitError> {
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

/// One line per request: method, path, status, and how long it took.
///
/// One line is the point. A span-based layer spreads the same facts across a
/// start, a finish and an end-of-stream event, which is three times the access
/// log for nothing.
#[cfg(feature = "ssr")]
async fn log_request(
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

/// The board changes constantly and is read by a command, not a cache, so
/// nothing along the way may hold on to it.
#[cfg(feature = "ssr")]
const WALL_HEADERS: [(axum::http::HeaderName, &str); 2] = [
    (
        axum::http::header::CONTENT_TYPE,
        "text/plain; charset=utf-8",
    ),
    (axum::http::header::CACHE_CONTROL, "no-store"),
];

/// Who is writing, for the rate limit.
///
/// Cloudflare sets `CF-Connecting-IP` and strips whatever the client sent, so
/// this is trustworthy behind the proxy and nowhere else. Anything reaching the
/// origin directly could claim any address it liked, which is why the origin is
/// not meant to be reachable directly.
#[cfg(feature = "ssr")]
fn writer(headers: &axum::http::HeaderMap) -> &str {
    headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("direct")
}

/// Handles one write to the board: validate, rate limit, apply, persist.
#[cfg(feature = "ssr")]
async fn write_cell(
    wall: &webpages::wall::State,
    headers: &axum::http::HeaderMap,
    body: &str,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse as _;

    let Some((x, y, byte)) = webpages::wall::parse_write(body) else {
        return (
            StatusCode::BAD_REQUEST,
            WALL_HEADERS,
            format!(
                "expected `x y char`, with x under {}, y under {}, and one printable \
                 character, or nothing to clear the cell\n",
                webpages::wall::COLS,
                webpages::wall::ROWS
            ),
        )
            .into_response();
    };

    if !wall.allowed(writer(headers)) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            WALL_HEADERS,
            // Which of the two limits was hit is not worth working out to say:
            // either way the answer is to write less.
            format!(
                "slow down: {} cells a minute each, {} across everyone\n",
                webpages::wall::BUDGET,
                webpages::wall::CEILING,
            ),
        )
            .into_response();
    }

    // Outside the lock, and only when something changed, so setting a cell to
    // what it already held costs no disk at all.
    if let Some(text) = wall.set(x, y, byte) {
        match tokio::fs::write(wall.path(), &text).await {
            Ok(()) => wall.saved(),
            // It still stands in memory, so the board is right until a
            // restart. Saying so beats failing the request.
            Err(error) => {
                tracing::error!(path = %wall.path().display(), %error, "could not persist the board");
            }
        }
    }

    (WALL_HEADERS, wall.render()).into_response()
}

/// Installs the termination handlers up front, so failing to is a startup
/// error rather than a shutdown that never comes. The future resolves on the
/// first signal.
///
/// systemd sends SIGTERM on stop and restart; SIGINT is Ctrl-C.
///
/// # Errors
///
/// Whatever stopped the handlers being installed.
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

    // First, so every failure below has somewhere to go.
    if let Err(error) = init_tracing() {
        warn!(%error, "keeping the tracing subscriber already installed");
    }
    init_panic_logging();

    // Pins the start before anything can ask for the uptime.
    let _ = webpages::clock::uptime_secs();

    // Leptos renders through a global spawner, which `leptos_axum` only
    // installs from its own router helpers. Serving one route by hand skips
    // that, and every render panics on its first spawn. An error means one is
    // already set, which is just as good.
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

    // The quiet bad deploy: it starts fine, but `LEPTOS_SITE_ROOT` points
    // nowhere and every page renders without CSS or wasm.
    if !std::path::Path::new(&*leptos_options.site_root).is_dir() {
        warn!(
            site_root = %leptos_options.site_root,
            "site root is not a directory, static assets will 404",
        );
    }

    // The other one, and the one that actually bit. Leptos resolves the
    // hashed bundle names through this file and looks for it beside the
    // binary, not under the site root. Leave it behind and every render panics
    // on an unguarded read, the connection drops, and a proxy answers 502.
    //
    // Logged rather than fatal: the file is read lazily on the first render, so
    // refusing to start would only move the same failure earlier. Saying it
    // here is what puts a cause next to the 502.
    if leptos_options.hash_files {
        let beside_binary = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(&*leptos_options.hash_file)));

        match beside_binary {
            Some(path) if path.is_file() => {}
            // Naming the path is the point: otherwise this looks the same as
            // the file simply being somewhere else.
            Some(path) => error!(
                path = %path.display(),
                "hash file not found, every render will panic and serve nothing; \
                 put it there or point LEPTOS_HASH_FILE_NAME at it",
            ),
            None => error!("cannot locate this binary, so the hash file cannot be found"),
        }
    }

    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(error) => {
            error!(%error, "could not install the signal handlers");
            return ExitCode::FAILURE;
        }
    };

    // One page, so one URL. Built from `SITE_URL` rather than written out
    // again, and leaked because it never changes.
    let sitemap: &'static str = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{}</loc></url></urlset>"#,
        webpages::SITE_URL
    )
    .leak();

    // The board outlives any one release, so it cannot sit in the release
    // directory: a deploy swaps that out and would take the board with it.
    // `WALL_PATH` points somewhere that survives, which for the real deploy is
    // a directory in a home the service can reach. See the unit.
    let wall = std::sync::Arc::new(webpages::wall::State::load(
        std::env::var_os("WALL_PATH")
            .map_or_else(|| std::path::PathBuf::from("wall.txt"), Into::into),
    ));

    // The board is written on every change, so a path that cannot be written
    // loses everything anyone draws. Say so now rather than at the first write.
    if let Err(error) = wall.persist() {
        error!(
            path = %wall.path().display(),
            %error,
            "cannot write the graffiti board, so nothing drawn on it will be kept",
        );
    }

    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            error!(address = %addr, %error, "could not bind the listener");
            return ExitCode::FAILURE;
        }
    };
    info!(
        // The socket's own address, which is not the configured one when that
        // asked for port 0 or an unspecified host.
        address = %listener.local_addr().unwrap_or(addr),
        build = webpages::BUILD,
        site_root = %leptos_options.site_root,
        site_pkg_dir = %leptos_options.site_pkg_dir,
        wall = %wall.path().display(),
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
            "/api/wall",
            get({
                let wall = std::sync::Arc::clone(&wall);
                move || {
                    let wall = std::sync::Arc::clone(&wall);
                    async move { (WALL_HEADERS, wall.render()) }
                }
            })
            .post({
                let wall = std::sync::Arc::clone(&wall);
                move |headers: axum::http::HeaderMap, body: String| {
                    let wall = std::sync::Arc::clone(&wall);
                    async move { write_cell(&wall, &headers, &body).await }
                }
            }),
        )
        .route(
            "/health",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    format!(
                        r#"{{"status":"ok","uptime_seconds":{},"build":"{}"}}"#,
                        webpages::clock::uptime_secs(),
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
        // Everything under the site root, and a 404 for anything else. The
        // pages the old site had are gone, so they land here.
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

/// The wasm build has no server to be. This exists so the crate has a `main`.
#[cfg(not(feature = "ssr"))]
pub const fn main() {}
