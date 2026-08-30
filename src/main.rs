//! The server binary. Everything it does is in `webpages::server`; this is the
//! entry point and the runtime it runs on.

// Nothing in the server needs unsafe, so nothing may bring it back.
#![forbid(unsafe_code)]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> std::process::ExitCode {
    webpages::server::run().await
}

/// The wasm build has no server to be. This exists so the crate has a `main`.
#[cfg(not(feature = "ssr"))]
pub const fn main() {}
