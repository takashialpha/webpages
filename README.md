# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, served live at **[takashialpha.com](https://takashialpha.com)**. A server-rendered Rust app built on a [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum) stack, with hydration on the client.

## Prerequisites

1. WASM target: `rustup target add wasm32-unknown-unknown`
2. `cargo install cargo-leptos`

## Running locally

```sh
cargo leptos serve --release
```

The server listens on `[::1]:3000` (IPv6 loopback only) by default, configurable via `site-addr` in `Cargo.toml` or the `LEPTOS_SITE_ADDR` environment variable.

## Building for release

```sh
cargo leptos build --release
```

Produces the server binary in `target/release` and the site assets in `target/site`. Bundle filenames are content hashed, so `target/release/hash.txt` is written alongside the binary to map each one; the server reads it to build the `/pkg` URLs.

## Pre-built binary

Every push to `main` publishes a rolling [`build`](https://github.com/takashialpha/webpages/releases/tag/build) release carrying `webpages-x86_64-linux.tar.gz` and its `.sha256`. The archive holds the server binary next to a `site` directory, already laid out the way the environment variables below expect. There are no version tags; the latest build is always whatever is in `main`.

## Deploying

After `cargo leptos build --release`, copy the server binary, `target/release/hash.txt`, and the `target/site` directory to the target host, keeping `hash.txt` in the same directory as the binary. Then set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
```

and run the binary.

The server shuts down on SIGINT or SIGTERM. It stops accepting new connections and waits for the requests already in flight to finish before it exits, so a restart never cuts a page off mid render. It exits with a non-zero status if it fails to start, and says why.

## Logging

The server writes plain text logs to stdout. `RUST_LOG` sets the verbosity, defaulting to `warn,webpages=info` when it is unset: the startup and shutdown lines, plus any warnings and errors.

Request logging sits one level below that, so it is off unless asked for:

```sh
export RUST_LOG="webpages=debug"
```

Each request then gets a single line with its method, path, status, and how long it took.

The format follows where stdout goes. Color is used only when stdout is a terminal, and the timestamp is left out when systemd owns stdout, because journald records its own timestamp for every line.

At startup the server also warns about the two ways a deploy can come up looking healthy while serving broken pages: `LEPTOS_SITE_ROOT` not pointing at a directory, and `hash.txt` missing from beside the binary.

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
