//! The server: what it checks before it starts, what it serves, and how it
//! stops.
//!
//! In the library rather than the binary, so `main.rs` is only an entry point.

mod board;
mod logging;
mod routes;
mod shutdown;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use leptos::config::get_configuration;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::wall;

/// Brings the server up and serves until a signal arrives.
///
/// Everything that can go wrong here is a startup problem, and each one says
/// what it was before this returns a failing status.
pub async fn run() -> ExitCode {
    // First, so every failure below has somewhere to go.
    if let Err(error) = logging::init_tracing() {
        warn!(%error, "keeping the tracing subscriber already installed");
    }
    logging::init_panic_logging();

    // Pins the start before anything can ask for the uptime.
    let _ = crate::clock::uptime_secs();

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

    check_assets(&leptos_options);

    let shutdown = match shutdown::on_signal() {
        Ok(shutdown) => shutdown,
        Err(error) => {
            error!(%error, "could not install the signal handlers");
            return ExitCode::FAILURE;
        }
    };

    let wall = Arc::new(load_board());

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
        build = crate::BUILD,
        site_root = %leptos_options.site_root,
        site_pkg_dir = %leptos_options.site_pkg_dir,
        wall = %wall.path().display(),
        "listening",
    );

    if let Err(error) = axum::serve(listener, routes::router(leptos_options, wall))
        .with_graceful_shutdown(shutdown)
        .await
    {
        error!(%error, "server stopped");
        return ExitCode::FAILURE;
    }

    info!("shutdown complete");
    ExitCode::SUCCESS
}

/// The two ways a deploy comes up clean and serves nothing.
///
/// Both are logged rather than fatal. The files are read lazily, on the first
/// render, so refusing to start would only move the same failure earlier; the
/// point of saying it here is to put a cause next to what the visitor sees.
fn check_assets(options: &leptos::config::LeptosOptions) {
    // The quiet one: it starts fine, but `LEPTOS_SITE_ROOT` points nowhere and
    // every page renders without CSS or wasm.
    if !Path::new(&*options.site_root).is_dir() {
        warn!(
            site_root = %options.site_root,
            "site root is not a directory, static assets will 404",
        );
    }

    // The loud one, and the one that actually bit. Leptos resolves the hashed
    // bundle names through this file and looks for it beside the binary, not
    // under the site root. Leave it behind and every render panics on an
    // unguarded read, the connection drops, and a proxy answers 502.
    if !options.hash_files {
        return;
    }
    let beside_binary = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&*options.hash_file)));

    match beside_binary {
        Some(path) if path.is_file() => {}
        // Naming the path is the point: otherwise this looks the same as the
        // file simply being somewhere else.
        Some(path) => error!(
            path = %path.display(),
            "hash file not found, every render will panic and serve nothing; \
             put it there or point LEPTOS_HASH_FILE_NAME at it",
        ),
        None => error!("cannot locate this binary, so the hash file cannot be found"),
    }
}

/// Opens the graffiti board, and says so now if it cannot be written.
///
/// It outlives any one release, so it cannot sit in the release directory: a
/// deploy swaps that out and would take the board with it. `WALL_PATH` points
/// somewhere that survives, which for the real deploy is a directory in a home
/// the service can reach. See the unit.
fn load_board() -> wall::State {
    let wall = wall::State::load(
        std::env::var_os("WALL_PATH").map_or_else(|| PathBuf::from("wall.txt"), Into::into),
    );

    // Written on every change, so a path that cannot be written loses
    // everything anyone draws. Say so now rather than at the first write.
    if let Err(error) = wall.persist() {
        error!(
            path = %wall.path().display(),
            %error,
            "cannot write the graffiti board, so nothing drawn on it will be kept",
        );
    }
    wall
}
